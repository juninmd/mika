//! Estado do shell que o Tauri não expõe por getter.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use tauri::{AppHandle, Manager};

use crate::config::store::ConfigStore;
use crate::native::{self, NativeRegistry};

pub struct ShellState {
    /// O ÚNICO `ConfigStore` do processo. Ver `config_store()` para o porquê de ser um só.
    ///
    /// `OnceLock` porque o store precisa do `app_data_dir`, que só existe depois que o Tauri
    /// monta o app — e `ShellState::new()` roda antes disso, no `.manage()`.
    config: OnceLock<Arc<ConfigStore>>,
    /// Os módulos de backend que já vivem em Rust. O que eles não reivindicam desce para o
    /// sidecar — ver `native/mod.rs` e `docs/tauri-rust-migration.md`.
    pub native: NativeRegistry,
    /// A janela nasce com `always_on_top(true)`; Tauri não tem getter para isso.
    always_on_top: AtomicBool,
    cursor_polling: AtomicBool,
    /// "O usuário pediu para sair" — faz o fechar da janela encerrar em vez de esconder.
    pub quitting: AtomicBool,
    /// Reentrância do gate de encerramento. Separado de `quitting` de propósito: sair pela
    /// bandeja marca `quitting` ANTES do ExitRequested, e um flag só faria o gate concluir
    /// que já tinha rodado e pular os shutdown hooks do backend.
    pub shutdown_started: AtomicBool,
    /// A janela do avatar já foi revelada (pelo sidecar ou pelo prazo de segurança).
    pub revealed: AtomicBool,
    /// O backend já DECIDIU o que fazer no boot — revelar o avatar **ou** abrir o onboarding.
    ///
    /// Separado de `revealed` de propósito. O prazo de segurança olhava `revealed`, então abrir o
    /// wizard o deixava falso e, 8s depois, o avatar aparecia por cima do onboarding: o primeiro
    /// boot mostrava as duas janelas. Marcar `revealed` no `openWizard` seria pior — é ele que dá
    /// a idempotência de `reveal_main`, e o avatar nunca mais apareceria ao fim do onboarding.
    pub boot_resolved: AtomicBool,
}

impl ShellState {
    pub fn new() -> Self {
        Self {
            config: OnceLock::new(),
            native: native::registry(),
            always_on_top: AtomicBool::new(true),
            cursor_polling: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
            shutdown_started: AtomicBool::new(false),
            revealed: AtomicBool::new(false),
            boot_resolved: AtomicBool::new(false),
        }
    }

    /// Só para teste de integração com `tauri::test::mock_app()` (ex.:
    /// `modules/settings/mod.rs::tests::config_changed_emit`). Pré-preenche o `OnceLock` com um
    /// `ConfigStore` já aberto pelo próprio teste (tempdir isolado), para `config_store(app)` nunca
    /// cair no ramo que lê `MIKA_PROFILE_DIR`/`app.path().app_data_dir()` — mutar essa env var pra
    /// testar mexeria no ambiente do processo `cargo test`, compartilhado por todo teste rodando em
    /// paralelo (mesma armadilha documentada em `e2e_hooks.rs`/`events/bus.rs`).
    #[cfg(test)]
    pub fn for_test_with_config_store(store: std::sync::Arc<crate::config::store::ConfigStore>) -> Self {
        let state = Self::new();
        let _ = state.config.set(store);
        state
    }
}

/// O `ConfigStore` do processo — um só, compartilhado por todos os módulos nativos.
///
/// **Por que não cada módulo abrir o seu.** `ConfigStore` guarda um cache do JSON inteiro em
/// memória (`cache: Mutex<Value>`) e salva o documento COMPLETO a cada `set`. Duas instâncias são
/// dois caches: a segunda grava por cima do documento que a primeira acabou de escrever, com os
/// valores que ela tinha lido antes — a escrita da primeira some, sem erro e sem log. A primeira
/// rodada da migração chegou exatamente aí: `settings` e `telemetry` abriram, cada um, um
/// `OnceLock<ConfigStore>` próprio. Nenhum teste de módulo pode ver isso, porque cada módulo,
/// sozinho, está correto. Quem impede a volta é `single_config_store_per_process`.
///
/// Devolve `Arc` e não `&ConfigStore` de propósito: o `&` viria emprestado do guard de
/// `app.state()`, e o chamador teria de manter o guard vivo à mão em cada função.
pub fn config_store(app: &AppHandle) -> Result<Arc<ConfigStore>, String> {
    let state = app.state::<ShellState>();

    if let Some(store) = state.config.get() {
        return Ok(Arc::clone(store));
    }

    let data_dir = crate::state::app_data_dir(app)
        .map_err(|error| format!("não resolveu app_data_dir: {error}"))?;
    // Perfil novo = sem `config.json` ainda (este é o primeiro `open` do processo). Perfil
    // existente nunca é tocado: mantém o `app.locale` que já tinha, ou o default de sempre.
    let first_run = !data_dir.join("config.json").exists();
    let store = ConfigStore::open(&data_dir, crate::config::schemas::registry())?;
    if first_run {
        let tag = crate::config::locale::os_locale_tag();
        if let Err(error) = crate::config::locale::seed_first_run(&store, tag.as_deref()) {
            log::warn!("idioma do SO não gravado: {error}");
        }
    }

    // Numa corrida, `set` guarda o primeiro e descarta o resto — e é justamente por isso que se
    // lê de volta do `OnceLock`: todo mundo sai com a MESMA instância, que é a invariante aqui.
    let _ = state.config.set(Arc::new(store));
    state
        .config
        .get()
        .map(Arc::clone)
        .ok_or_else(|| "config store não inicializou".to_string())
}

pub fn always_on_top(app: &AppHandle) -> bool {
    app.state::<ShellState>().always_on_top.load(Ordering::Relaxed)
}

pub fn set_always_on_top(app: &AppHandle, value: bool) {
    app.state::<ShellState>()
        .always_on_top
        .store(value, Ordering::Relaxed);
}

pub fn cursor_polling_running(app: &AppHandle) -> bool {
    app.state::<ShellState>()
        .cursor_polling
        .load(Ordering::Relaxed)
}

pub fn set_cursor_polling(app: &AppHandle, value: bool) {
    app.state::<ShellState>()
        .cursor_polling
        .store(value, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    /// Nenhum módulo nativo pode abrir o PRÓPRIO `ConfigStore`: tem de pedir o do `ShellState`.
    ///
    /// Este teste lê o fonte porque a falha que ele pega não aparece em runtime de teste nenhum.
    /// Duas instâncias são dois caches do JSON inteiro; a segunda a salvar apaga a escrita da
    /// primeira, e cada módulo, isolado, passa em todos os seus testes. Foi assim que `settings` e
    /// `telemetry` chegaram — cada um com seu `OnceLock<ConfigStore>`, cada um correto sozinho.
    ///
    /// O corte em `#[cfg(test)]` é obrigatório: os testes de cada módulo abrem stores temporários
    /// de propósito, e contá-los faria o teste acusar o uso legítimo.
    ///
    /// O corte de comentário também: a primeira versão deste teste casava a string crua e acusou
    /// `brain/turn.rs` por causa do comentário que AVISA para não usar `ConfigStore::open`. Gate que
    /// lê prosa como se fosse código pune quem documenta e continua cego a quem esconde a chamada.
    #[test]
    fn single_config_store_per_process() {
        let modules = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/modules");
        let mut read = 0usize;
        let mut offenders = Vec::new();

        /// A chamada, não a menção: o que vem depois de `//` na linha é comentário.
        fn is_real_call(line: &str) -> bool {
            line.split("//").next().unwrap_or_default().contains("ConfigStore::open")
        }

        // Prova de que o corte de comentário não cegou a busca — as duas formas que já apareceram
        // aqui, uma legítima e uma não, conferidas a cada rodada em vez de confiadas.
        assert!(is_real_call("    let store = ConfigStore::open(&dir, registry())?;"));
        assert!(!is_real_call("/// nunca `ConfigStore::open` — peça o do ShellState"));

        // A isenção de teste DISCRIMINA — não é o nome do arquivo que compra passe livre. As três
        // formas reais desta árvore, conferidas contra o disco a cada rodada:
        let modules_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("modules");
        let inner_attr = modules_root.join("privacy").join("tests.rs");
        let declared = modules_root.join("browser").join("guarded_fetch").join("tests.rs");
        assert!(
            is_test_only(&inner_attr, &fs::read_to_string(&inner_attr).expect("privacy/tests.rs")),
            "`#![cfg(test)]` interno deixou de ser reconhecido — privacy/tests.rs voltaria a ser lido como produção"
        );
        assert!(
            is_test_only(&declared, &fs::read_to_string(&declared).expect("guarded_fetch/tests.rs")),
            "`#[cfg(test)] mod tests;` no pai deixou de ser reconhecido"
        );
        assert!(
            !is_test_only(Path::new("src/modules/anything/tests.rs"), "let store = ConfigStore::open(&dir, registry())?;"),
            "um tests.rs sem pai legível virou isento pelo nome — a isenção tem de ser provada, não presumida"
        );
        let path_declared = modules_root.join("settings").join("plugin_test_tests.rs");
        assert!(
            is_test_only(&path_declared, &fs::read_to_string(&path_declared).expect("settings/plugin_test_tests.rs")),
            "`#[path = \"...\"]` deixou de ser reconhecido — toda suíte que saiu para arquivo próprio \
             com nome diferente de `tests.rs` volta a ser lida como produção"
        );
        // Sintético, porque o que importa aqui é a NEGATIVA: `#[path]` sozinho não isenta. Um
        // arquivo incluído em produção por `#[path]` é produção, tenha o nome que tiver.
        assert!(declares_cfg_test_mod("#[cfg(test)]\n#[path = \"x_tests.rs\"]\nmod tests;", "x_tests.rs", "x_tests"));
        assert!(declares_cfg_test_mod("#[path = \"x_tests.rs\"]\n#[cfg(test)]\nmod tests;", "x_tests.rs", "x_tests"));
        assert!(
            !declares_cfg_test_mod("#[path = \"x_tests.rs\"]\nmod tests;", "x_tests.rs", "x_tests"),
            "`#[path]` sem `#[cfg(test)]` comprou isenção — arquivo compilado em produção passaria batido"
        );

        /// Um arquivo é teste inteiro de TRÊS formas, todas em uso nesta árvore, e nenhuma delas
        /// deixa `#[cfg(test)]` no meio do arquivo para o corte de baixo encontrar:
        ///
        /// 1. `#![cfg(test)]` interno, na primeira linha de código (`modules/privacy/tests.rs`,
        ///    `modules/gamer/test_support.rs` — e o cabeçalho do primeiro diz, com todas as letras,
        ///    que escolheu essa forma POR CAUSA deste gate);
        /// 2. `#[cfg(test)] mod tests;` na declaração, no módulo pai — `<dir>/mod.rs` ou `<dir>.rs`,
        ///    as duas formas que o Rust aceita;
        /// 3. `#[cfg(test)] #[path = "<arquivo>"] mod tests;` em QUALQUER irmão do mesmo diretório.
        ///
        /// A forma 3 entrou quando a regra de 300 linhas obrigou várias suítes a sair para arquivo
        /// próprio com nome descritivo (`rpc_tests.rs`, `plugin_test_tests.rs`, `store_tests.rs`).
        /// Este gate reconhecia a isenção pelo NOME `tests.rs`, então acusou todas elas de abrir
        /// `ConfigStore` em produção — sendo que o `ConfigStore` delas é de teste, e o arquivo nem
        /// compila fora de `cfg(test)`. Gate que trata convenção de nome como prova vermelha na
        /// primeira vez que a convenção muda, e o custo é ele ser desligado.
        ///
        /// Não conseguir provar nenhuma das três devolve `false`: na dúvida conta como produção,
        /// que é o lado seguro para um gate cuja função é barrar segundo `ConfigStore` no processo.
        fn is_test_only(path: &Path, source: &str) -> bool {
            if source.lines().any(|line| line.trim() == "#![cfg(test)]") {
                return true;
            }
            let (Some(dir), Some(file_name), Some(stem)) =
                (path.parent(), path.file_name().and_then(|n| n.to_str()), path.file_stem().and_then(|n| n.to_str()))
            else {
                return false;
            };

            // `#[path]` é relativo ao diretório de quem declara, então o declarante é sempre um
            // irmão; `<dir>.rs` entra à parte por ser a outra forma de módulo que o Rust aceita.
            fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .chain(std::iter::once(dir.with_extension("rs")))
                .filter(|candidate| candidate != path && candidate.extension().is_some_and(|ext| ext == "rs"))
                .filter_map(|candidate| fs::read_to_string(candidate).ok())
                .any(|parent_source| declares_cfg_test_mod(&parent_source, file_name, stem))
        }

        /// `#[cfg(test)]` e `#[path = "..."]` vêm em linhas separadas e em qualquer ordem, então a
        /// checagem é do BLOCO de atributos que precede o `mod ...;` — nunca da linha anterior
        /// sozinha, que era o que a versão anterior olhava.
        fn declares_cfg_test_mod(parent_source: &str, file_name: &str, stem: &str) -> bool {
            let lines: Vec<&str> = parent_source.lines().collect();
            lines.iter().enumerate().any(|(index, line)| {
                let declaration = line.trim_start();
                if !declaration.starts_with("mod ") || !declaration.ends_with(';') {
                    return false;
                }
                let declares_by_name = declaration == format!("mod {stem};");
                let (mut cfg_test, mut declares_by_path) = (false, false);
                let mut cursor = index;
                while cursor > 0 && lines[cursor - 1].trim_start().starts_with("#[") {
                    let attribute = lines[cursor - 1].trim();
                    cfg_test |= attribute == "#[cfg(test)]";
                    declares_by_path |= attribute.starts_with("#[path") && attribute.contains(file_name);
                    cursor -= 1;
                }
                cfg_test && (declares_by_name || declares_by_path)
            })
        }

        // Recursivo: módulo grande vira diretório (a regra de 300 linhas obriga), e um submódulo
        // que abrisse o próprio store passaria despercebido por uma varredura só da raiz.
        fn walk(dir: &Path, read: &mut usize, offenders: &mut Vec<String>) {
            for entry in fs::read_dir(dir).expect("src/modules não existe") {
                let path = entry.expect("entrada ilegível").path();

                if path.is_dir() {
                    walk(&path, read, offenders);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs") {
                    continue;
                }

                let source = fs::read_to_string(&path).expect("módulo ilegível");
                *read += 1;

                // Um `tests.rs` é teste INTEIRO: o `#[cfg(test)]` dele mora na declaração, no
                // módulo pai, não dentro do arquivo. Sem esta exceção o corte abaixo não encontra
                // marcador nenhum e o arquivo todo passa por produção — que é exatamente como este
                // gate acusou dois falsos positivos quando a regra de 300 linhas levou suítes para
                // arquivo próprio. A exceção é VERIFICADA, não presumida: só vale se o pai declarar
                // com `#[cfg(test)]`. Um `tests.rs` declarado sem isso compila em produção de
                // verdade, e aí o nome do arquivo não compra isenção nenhuma.
                if is_test_only(&path, &source) {
                    continue;
                }

                let production = source.split("#[cfg(test)]").next().unwrap_or_default();
                if production.lines().any(is_real_call) {
                    offenders.push(path.file_name().unwrap().to_string_lossy().into_owned());
                }
            }
        }

        walk(&modules, &mut read, &mut offenders);

        // Piso de alcance: `read_dir` devolvendo pouca coisa (pasta movida, filtro quebrado) faria
        // o teste passar sem ter lido os módulos — o verde falso que ele existe para impedir.
        assert!(read >= 12, "só {read} módulo(s) lido(s) em src/modules — leitura quebrada");

        assert!(
            offenders.is_empty(),
            "módulo(s) abrindo ConfigStore próprio: {offenders:?} — use crate::state::config_store(app)"
        );
    }
}

/// Diretório de dados do app. `MIKA_PROFILE_DIR` substitui o padrão do SO — é o equivalente do
/// `--user-data-dir` do Electron, e sem ele o E2E rodava sobre o perfil REAL do usuário.
pub fn app_data_dir<R: tauri::Runtime, M: Manager<R>>(app: &M) -> tauri::Result<std::path::PathBuf> {
    match profile_override(std::env::var_os("MIKA_PROFILE_DIR")) {
        Some(dir) => Ok(dir),
        None => app.path().app_data_dir(),
    }
}

fn profile_override(value: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    value.filter(|dir| !dir.is_empty()).map(Into::into)
}

#[cfg(test)]
mod profile_dir_tests {
    use super::profile_override;

    #[test]
    fn only_a_non_empty_override_replaces_the_os_data_dir() {
        assert_eq!(profile_override(None), None);
        assert_eq!(profile_override(Some("".into())), None);
        assert_eq!(profile_override(Some("C:/p".into())), Some("C:/p".into()));
    }
}
