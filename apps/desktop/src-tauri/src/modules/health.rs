//! Módulo `health` em Rust — port de `ipc/health.ts` (`mika:get-health`) e `ipc/prompts.ts`
//! (`mika:prompts-list`/`-save`/`-reset`).
//!
//! `mika:get-health` é o smoke do app (ver `e2e/specs/boot.spec.ts`): enquanto o feature
//! registry e o DI do LLM/TTS/STT continuarem só no sidecar Node, este módulo não pode inventar
//! esses números — cada campo que Rust ainda não sabe vem `0`/`false` com o porquê no comentário,
//! não um valor forjado para o gate passar calado.
//!
//! Os prompts em si (mesmo objetivo de `packages/sdk/src/prompts/registry.ts`) leem do recurso
//! empacotado `prompts/` — é a MESMA pasta que `tauri.conf.json` copia de `../out/prompts`
//! (ver `build/build-prompts.mts`), nas duas
//! formas: achatada (ignorada aqui) e por namespace (o que este módulo enumera).

use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;
use crate::state::ShellState;

const CHANNELS: &[&str] = &[
    "mika:get-health",
    "mika:prompts-list",
    "mika:prompts-save",
    "mika:prompts-reset",
];

/// Espelha `PROMPT_OVERRIDE_MAX_CHARS` de `desktop-ipc-plugin-schemas.ts`. Duplicado porque o
/// alvo Tauri não passa pelo router/Zod do Electron — `bridge.rs` resolve este canal direto, sem
/// validação de schema no caminho. O teste `max_chars_matches_ts_constant` prende os dois juntos.
const PROMPT_OVERRIDE_MAX_CHARS: usize = 64_000;

struct HealthModule {
    /// Capturado na montagem do `NativeRegistry`, bem cedo no boot — não é o instante exato de
    /// `main()`, mas é a aproximação mais honesta disponível sem instrumentar o processo inteiro.
    start: Instant,
}

#[async_trait]
impl NativeModule for HealthModule {
    fn name(&self) -> &'static str {
        "health"
    }

    fn channels(&self) -> &'static [&'static str] {
        CHANNELS
    }

    async fn invoke(
        &self,
        app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        match channel {
            "mika:get-health" => {
                if is_owner_surface(ctx.window_type) {
                    crate::modules::llm::status::arm(app);
                }
                // Alcance real do cérebro (Ollama respondeu / provedor remoto tem credencial), não
                // mais `true` fixo — ver o cabeçalho de `llm/status.rs`.
                let has_llm = match crate::state::config_store(app) {
                    Ok(store) => crate::modules::llm::status::has_llm(&store).await,
                    Err(_) => false,
                };
                Ok(self.health(app, has_llm))
            }
            "mika:prompts-list" => {
                guard_owner(ctx)?;
                Ok(json!({ "prompts": list_prompts(app) }))
            }
            "mika:prompts-save" => {
                guard_owner(ctx)?;
                Ok(save_prompt(app, payload))
            }
            "mika:prompts-reset" => {
                guard_owner(ctx)?;
                Ok(reset_prompt(app, payload))
            }
            other => Err(format!("canal '{other}' declarado em `health` mas não tratado")),
        }
    }
}

/// `"steam"` na build `--features steam` (o updater do Tauri nem é registrado), `"direct"` no resto.
/// Os dois literais são o enum `updateChannel` de `desktopHealthSchema` (teste abaixo).
fn update_channel() -> &'static str {
    if cfg!(feature = "steam") {
        "steam"
    } else {
        "direct"
    }
}

impl HealthModule {
    fn health(&self, app: &AppHandle, has_llm: bool) -> Value {
        let state = app.state::<ShellState>();
        let revealed = state.revealed.load(Ordering::Relaxed);
        let boot_resolved = state.boot_resolved.load(Ordering::Relaxed);

        // Únicos sinais de boot que o Rust realmente tem hoje: `revealed` (avatar mostrado) e
        // `boot_resolved` (backend decidiu avatar-ou-wizard). Não dá pra distinguir
        // coreMinimumReady de pluginsReady — essa granularidade ainda vive só em
        // `bootstrap-state.ts` do sidecar. `degraded`, porém, TEM fonte própria desde que
        // `modules/mod.rs::register_config_bound_tools` passou a reportar cada `Err` de registro a
        // `crate::feature_health` em vez de só logar — ver o cabeçalho de `feature_health.rs`.
        //
        // `status` (não só `bootstrap.degraded`) precisa virar `"degraded"`: é o campo que
        // `BootHealthBanner.tsx` olha (`health?.status !== 'degraded'`) e que
        // `desktop-ipc-core-schemas.ts` já declarava no enum sem ninguém emitir — sem isto o
        // banner nunca aparecia, mesmo com uma feature caída.
        let registration_errors = crate::feature_health::registration_failures();
        let status = if !registration_errors.is_empty() {
            "degraded"
        } else if revealed {
            "ok"
        } else if boot_resolved {
            "starting"
        } else {
            "initializing"
        };

        let features = crate::feature_registry::shared().get_all().len();
        json!({
            "status": status,
            "uptime": self.start.elapsed().as_secs(),
            "features": features,
            "settingsTabs": crate::feature_registry::shared().get_all_settings().len(),
            "tools": crate::modules::tools::registry().get_all().len(),
            "version": app.package_info().version.to_string(),
            // `tauri::is_dev()` é o equivalente direto de `app.isPackaged` do Electron: fica
            // `true` só quando o CLI liga a feature `custom-protocol` (ver Cargo.toml).
            "packaged": !tauri::is_dev(),
            // Quem atualiza o binário: o cliente Steam (SteamPipe) ou o updater do Tauri.
            "updateChannel": update_channel(),
            "bootstrap": bootstrap_json(revealed, registration_errors),
            "capabilities": {
                // TTS e STT são compilados no binário; o LLM depende de um servidor/credencial.
                "hasLLM": has_llm,
                "canSpeak": true,
                "canListenAudio": true,
                "canSeeScreen": crate::state::config_store(app)
                    .ok()
                    .and_then(|store| store.get("vision.enabled"))
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false),
                // Mesmos valores fixos de `health.ts`: lá também não derivam de estado nenhum.
                "canUseTerminal": true,
                "canLearn": true,
                "plugins": features,
            },
        })
    }
}

/// Puro de propósito — sem `AppHandle`, para ser testável sem `tauri::test::mock_app()` (o
/// `AppHandle` de `health()` não é genérico sobre `Runtime`, então não compila com
/// `AppHandle<MockRuntime>`). `degraded` é `true` sse alguma feature falhou ao registrar; sem
/// falha nenhuma o comportamento é idêntico ao de antes desta mudança (tudo `false`/`[]`).
fn bootstrap_json(revealed: bool, registration_errors: Vec<String>) -> Value {
    let degraded = !registration_errors.is_empty();
    json!({
        "coreMinimumReady": revealed,
        "pluginsReady": revealed && !degraded,
        "degraded": degraded,
        "errors": registration_errors,
    })
}

/// Mesma janela de `assertOwnerSurface`/`surface-policy.ts`: só main/settings/wizard mexem em
/// prompts. `get-health` fica de fora de propósito — `health.ts` nunca chama essa guarda para ele.
fn guard_owner(ctx: &InvokeContext) -> Result<(), String> {
    if is_owner_surface(ctx.window_type) {
        Ok(())
    } else {
        Err(format!("rota de prompts recusada para {:?}", ctx.window_type))
    }
}

/// Raiz dos prompts empacotados (`bundle.resources` -> `prompts/`). Fora do bundle (dev/
/// `cargo test`), cai no `out/prompts` cru que `build/build-prompts.mts` escreve. Até 2026-09-22
/// os dois caminhos eram `sidecar/prompts`, produzidos de carona pelo build do sidecar; apagá-lo
/// deixaria o `.md` de fora sem erro nenhum — prompt ausente vira string vazia, não exceção.
fn prompts_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(path) = app.path().resolve("prompts", BaseDirectory::Resource) {
        if path.is_dir() {
            return Some(path);
        }
    }

    let dev_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()?
        .join("out")
        .join("prompts");
    dev_root.is_dir().then_some(dev_root)
}

/// `<userData>/prompt-overrides/` — mesma raiz de `getPromptOverrideRoot()` em
/// `packages/sdk/src/prompts/override-root.ts`; aqui é o próprio Tauri quem resolve `userData`
/// (`app_data_dir`), porque não existe bootstrap Node chamando `setPromptOverrideRoot` neste alvo.
fn override_root(app: &AppHandle) -> Option<PathBuf> {
    crate::state::app_data_dir(app).ok().map(|dir| dir.join("prompt-overrides"))
}

/// Um segmento de caminho nunca vira travessia: sem `..`, sem separador alternativo, sem byte
/// nulo. `namespace` e cada trecho de `key` passam por aqui antes de tocar o disco.
fn is_safe_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && !segment.contains('\\')
        && !segment.contains(':')
        && !segment.contains('\0')
}

fn is_safe_namespace(namespace: &str) -> bool {
    is_safe_segment(namespace)
}

/// `key` pode ter subpastas (`awareness/game-absence.md`), sempre separadas por `/` — nunca por
/// `\`, mesmo no Windows: é a forma que `walkMarkdown` produz do lado TypeScript.
fn is_safe_key(key: &str) -> bool {
    !key.is_empty() && key.ends_with(".md") && key.split('/').all(is_safe_segment)
}

/// O ÚNICO ponto que decide se `namespace`/`key` pode virar caminho: segmento seguro E o arquivo
/// default realmente existe sob `root`. Isso substitui a whitelist do `registry.ts` (lá, o
/// registro de módulos; aqui, o próprio recurso empacotado é a fonte da verdade).
fn is_known_prompt_in(root: &Path, namespace: &str, key: &str) -> bool {
    is_safe_namespace(namespace) && is_safe_key(key) && root.join(namespace).join(key).is_file()
}

/// Caminhos relativos (`sub/nome.md`, sempre com `/`) de todo `.md` sob `dir` — mesma forma de
/// `walkMarkdown` em `registry.ts`, pro `key` bater dos dois lados do contrato.
fn walk_markdown(dir: &Path, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return out };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };

        if file_type.is_dir() {
            out.extend(walk_markdown(&entry.path(), &rel));
        } else if name.ends_with(".md") {
            out.push(rel);
        }
    }

    out
}

/// Nomes únicos de `{{placeholder}}`, na ordem da primeira ocorrência — mesmo contrato de
/// `placeholdersOf` em `registry.ts` (regex `/\{\{(\w+)\}\}/g` só ASCII, sem flag `u`).
fn placeholders_of(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;

    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start && end + 1 < bytes.len() && bytes[end] == b'}' && bytes[end + 1] == b'}' {
                // Só ASCII entre as chaves: fatiar aqui nunca cai no meio de um caractere UTF-8.
                let name = text[start..end].to_string();
                if !found.contains(&name) {
                    found.push(name);
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }

    found
}

fn placeholder_token(name: &str) -> String {
    let mut token = String::with_capacity(name.len() + 4);
    token.push_str("{{");
    token.push_str(name);
    token.push_str("}}");
    token
}

fn list_prompts(app: &AppHandle) -> Vec<Value> {
    // Sem recurso empacotado, a lista vazia é a resposta honesta — nunca um caminho chutado.
    let Some(root) = prompts_root(app) else { return Vec::new() };
    list_prompts_in(&root, override_root(app).as_deref())
}

fn list_prompts_in(root: &Path, override_base: Option<&Path>) -> Vec<Value> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root) else { return out };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else { continue };
        // Só as subpastas por namespace: os `.md` soltos na raiz são a cópia achatada que
        // `loadPrompt()` lê, e listá-los aqui misturaria módulos, igual `registry.ts` evita.
        if !file_type.is_dir() {
            continue;
        }
        let namespace = entry.file_name().to_string_lossy().into_owned();
        let namespace_dir = entry.path();

        for key in walk_markdown(&namespace_dir, "") {
            let Ok(raw_default) = fs::read_to_string(namespace_dir.join(&key)) else { continue };
            let default_text = raw_default.trim().to_string();

            let override_text = override_base.and_then(|base| {
                fs::read_to_string(base.join(&namespace).join(&key)).ok().map(|t| t.trim().to_string())
            });

            out.push(json!({
                "namespace": namespace,
                "key": key,
                "defaultText": default_text,
                "overrideText": override_text,
                "placeholders": placeholders_of(&default_text),
            }));
        }
    }

    out
}

fn save_prompt(app: &AppHandle, payload: Value) -> Value {
    let namespace = payload.get("namespace").and_then(Value::as_str).unwrap_or_default();
    let key = payload.get("key").and_then(Value::as_str).unwrap_or_default();
    let text = payload.get("text").and_then(Value::as_str).unwrap_or_default();

    let Some(root) = prompts_root(app) else {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": "Pasta de prompts não encontrada no pacote" });
    };

    match override_root(app) {
        Some(override_base) => save_prompt_in(&root, &override_base, namespace, key, text),
        None => {
            json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": "Raiz de override de prompt não resolvida" })
        }
    }
}

/// Núcleo puro de `promptsSave` — sem `AppHandle`, testável com diretórios temporários. Mesma
/// regra de `savePromptOverride` em `registry.ts`: texto vazio/só espaço vira reset, nunca grava
/// `.md` vazio (incidente conhecido: requisição de zero token → 400 → provedor desativado).
fn save_prompt_in(root: &Path, override_base: &Path, namespace: &str, key: &str, text: &str) -> Value {
    if !is_known_prompt_in(root, namespace, key) {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": format!("Prompt desconhecido: {namespace}/{key}") });
    }

    // Contado em unidades UTF-16 (`encode_utf16`), não em `chars()`: é o que `.length` do
    // JS/Zod mede do outro lado do contrato — um emoji fora do BMP conta 2 nos dois lados.
    if text.encode_utf16().count() > PROMPT_OVERRIDE_MAX_CHARS {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": "Prompt excede o tamanho máximo" });
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        remove_override(override_base, namespace, key);
        return json!({ "ok": true, "reset": true, "droppedPlaceholders": [] });
    }

    if crate::modules::chat::prompt::drops_safety_floor(namespace, key, trimmed) {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": "Este prompt precisa manter o piso de segurança (\"Safety floor\")" });
    }

    let target = override_base.join(namespace).join(key);
    let Some(parent) = target.parent() else {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": "Caminho de override inválido" });
    };
    if let Err(error) = fs::create_dir_all(parent) {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": error.to_string() });
    }
    if let Err(error) = fs::write(&target, trimmed) {
        return json!({ "ok": false, "reset": false, "droppedPlaceholders": [], "error": error.to_string() });
    }

    let default_text = fs::read_to_string(root.join(namespace).join(key)).unwrap_or_default();
    let dropped: Vec<String> = placeholders_of(default_text.trim())
        .into_iter()
        .filter(|p| !trimmed.contains(&placeholder_token(p)))
        .collect();

    json!({ "ok": true, "reset": false, "droppedPlaceholders": dropped })
}

fn remove_override(override_base: &Path, namespace: &str, key: &str) {
    let file = override_base.join(namespace).join(key);
    if file.exists() {
        let _ = fs::remove_file(file);
    }
}

fn reset_prompt(app: &AppHandle, payload: Value) -> Value {
    let namespace = payload.get("namespace").and_then(Value::as_str).unwrap_or_default();
    let key = payload.get("key").and_then(Value::as_str).unwrap_or_default();

    let Some(root) = prompts_root(app) else {
        return json!({ "ok": false, "error": "Pasta de prompts não encontrada no pacote" });
    };

    match override_root(app) {
        Some(override_base) => reset_prompt_in(&root, &override_base, namespace, key),
        None => json!({ "ok": false, "error": "Raiz de override de prompt não resolvida" }),
    }
}

/// Núcleo puro de `promptsReset` — apaga só o arquivo de override do usuário, nunca o default
/// empacotado (`root` só é lido para confirmar que o prompt é conhecido).
fn reset_prompt_in(root: &Path, override_base: &Path, namespace: &str, key: &str) -> Value {
    if !is_known_prompt_in(root, namespace, key) {
        return json!({ "ok": false, "error": format!("Prompt desconhecido: {namespace}/{key}") });
    }

    remove_override(override_base, namespace, key);
    json!({ "ok": true })
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(HealthModule { start: Instant::now() })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::WindowType;
    use std::sync::atomic::AtomicU32;

    fn ctx(window_type: WindowType) -> InvokeContext {
        InvokeContext {
            window_type,
            window_label: "test".into(),
            plugin_id: None,
        }
    }

    #[test]
    fn update_channel_is_a_value_of_the_typescript_health_contract() {
        let schemas = include_str!("../../../../../packages/sdk/src/contracts/desktop-ipc-core-schemas.ts");
        assert!(
            schemas.contains("updateChannel: z.enum(['steam', 'direct'])"),
            "desktopHealthSchema.updateChannel divergiu do enum Rust"
        );
        assert!(["steam", "direct"].contains(&update_channel()));
    }

    #[test]
    fn claims_exactly_the_four_channels() {
        assert_eq!(
            CHANNELS,
            &[
                "mika:get-health",
                "mika:prompts-list",
                "mika:prompts-save",
                "mika:prompts-reset",
            ]
        );
    }

    /// Mesmo padrão de `bridge.rs::shell_channels_match_typescript_contract`: um literal errado
    /// aqui não quebra a compilação, só faz o canal cair de volta pro sidecar em silêncio.
    #[test]
    fn channels_match_typescript_contract() {
        let contract = include_str!("../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        for channel in CHANNELS {
            assert!(
                contract.contains(&format!("'{channel}'")),
                "canal {channel} não existe em desktop-ipc-channels.ts"
            );
        }
    }

    /// Bug real do teste, não do valor: `{PROMPT_OVERRIDE_MAX_CHARS}` formata `64000` (Display não
    /// reproduz o `_` de separador numérico), mas o fonte TS escreve `64_000` — a comparação
    /// literal nunca batia mesmo com os dois lados corretos. Em vez de fixar a formatação (que
    /// quebraria de novo se o TS reformatar o número), extrai o literal numérico do TS, descarta
    /// `_` e compara o VALOR.
    #[test]
    fn max_chars_matches_ts_constant() {
        let schemas = include_str!("../../../../../packages/sdk/src/contracts/desktop-ipc-plugin-schemas.ts");
        let marker = "PROMPT_OVERRIDE_MAX_CHARS = ";
        let start = schemas.find(marker).expect("PROMPT_OVERRIDE_MAX_CHARS não declarada em desktop-ipc-plugin-schemas.ts") + marker.len();
        let literal: String = schemas[start..]
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '_')
            .collect();
        let ts_value: usize = literal.replace('_', "").parse().expect("literal numérico inválido");

        assert_eq!(
            ts_value, PROMPT_OVERRIDE_MAX_CHARS,
            "PROMPT_OVERRIDE_MAX_CHARS divergiu do contrato TS ({ts_value} != {PROMPT_OVERRIDE_MAX_CHARS})"
        );
    }

    /// Prova do gap de `specs/01-architecture.md`: sem falha nenhuma reportada, `degraded` fica
    /// `false` — mesmo comportamento de antes de `feature_health` existir.
    #[test]
    fn bootstrap_json_not_degraded_when_no_registration_errors() {
        let bootstrap = bootstrap_json(true, vec![]);
        assert_eq!(bootstrap["degraded"], json!(false));
        assert_eq!(bootstrap["pluginsReady"], json!(true));
        assert_eq!(bootstrap["errors"], json!(Vec::<String>::new()));
    }

    /// UMA falha de registro basta para `degraded` virar `true` — "uma única feature que não sobe
    /// já torna o boot degraded" (`specs/01-architecture.md`). Antes desta rodada este campo era
    /// fixo em `false`; este teste falhava contra o `health()` antigo.
    #[test]
    fn bootstrap_json_degraded_when_one_feature_fails() {
        let bootstrap = bootstrap_json(true, vec!["scheduler: ConfigStore indisponível".to_string()]);
        assert_eq!(bootstrap["degraded"], json!(true));
        assert_eq!(bootstrap["pluginsReady"], json!(false));
        assert_eq!(
            bootstrap["errors"],
            json!(["scheduler: ConfigStore indisponível"])
        );
    }

    /// `plugin-view` e `unknown` são recusadas — mesma lista de `assertOwnerSurface` em
    /// `surface-policy.ts` (`main`, `settings`, `wizard` passam; o resto não).
    #[test]
    fn owner_surface_matches_ts_policy() {
        assert!(guard_owner(&ctx(WindowType::Main)).is_ok());
        assert!(guard_owner(&ctx(WindowType::Settings)).is_ok());
        assert!(guard_owner(&ctx(WindowType::Wizard)).is_ok());
        assert!(guard_owner(&ctx(WindowType::PluginView)).is_err());
        assert!(guard_owner(&ctx(WindowType::Unknown)).is_err());
    }

    // -- Segurança de caminho -----------------------------------------------------------------

    #[test]
    fn rejects_directory_traversal_in_key() {
        assert!(!is_safe_key("../../etc/passwd.md"));
        assert!(!is_safe_key("sub/../../../secrets.md"));
    }

    #[test]
    fn rejects_absolute_and_drive_paths() {
        assert!(!is_safe_key("/etc/passwd.md"));
        assert!(!is_safe_key("C:/Windows/system32/x.md"));
        assert!(!is_safe_namespace("C:"));
    }

    #[test]
    fn rejects_backslash_and_null_byte() {
        assert!(!is_safe_key("sub\\x.md"));
        assert!(!is_safe_key("sub/x\0.md"));
    }

    #[test]
    fn rejects_non_markdown_key() {
        assert!(!is_safe_key("sub/x.txt"));
    }

    #[test]
    fn accepts_normal_nested_key() {
        assert!(is_safe_key("awareness/game-absence.md"));
        assert!(is_safe_namespace("brain"));
    }

    // -- Placeholders --------------------------------------------------------------------------

    #[test]
    fn extracts_unique_placeholders_in_order() {
        assert_eq!(
            placeholders_of("Olá {{name}}, hoje é {{day}} e {{name}} de novo"),
            vec!["name", "day"]
        );
    }

    #[test]
    fn ignores_unclosed_or_malformed_braces() {
        assert_eq!(placeholders_of("{{ola} {{}} {{1x}}"), vec!["1x"]);
    }

    // -- Fixture de arquivos (sem dependência só para teste, mesmo padrão de config/store.rs) ---

    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-health-test-{tag}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Dir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_file(dir: &Path, rel: &str, content: &str) {
        let target = dir.join(rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, content).unwrap();
    }

    #[test]
    fn lists_known_prompt_with_placeholders_and_no_override() {
        let root = Dir::new("list-root");
        write_file(root.path(), "brain/monologue.md", "Fala {{tema}} pra mim");
        let overrides = Dir::new("list-overrides");

        let prompts = list_prompts_in(root.path(), Some(overrides.path()));
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0]["namespace"], "brain");
        assert_eq!(prompts[0]["key"], "monologue.md");
        assert_eq!(prompts[0]["defaultText"], "Fala {{tema}} pra mim");
        assert_eq!(prompts[0]["overrideText"], Value::Null);
        assert_eq!(prompts[0]["placeholders"], json!(["tema"]));
    }

    #[test]
    fn lists_override_text_when_present() {
        let root = Dir::new("list-root-2");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("list-overrides-2");
        write_file(overrides.path(), "brain/monologue.md", "override do usuário");

        let prompts = list_prompts_in(root.path(), Some(overrides.path()));
        assert_eq!(prompts[0]["overrideText"], "override do usuário");
    }

    #[test]
    fn missing_prompts_root_returns_empty_list_not_a_guessed_path() {
        assert_eq!(list_prompts_in(Path::new("/rota/que/nao/existe"), None), Vec::<Value>::new());
    }

    #[test]
    fn save_rejects_unknown_prompt() {
        let root = Dir::new("save-root-unknown");
        let overrides = Dir::new("save-overrides-unknown");

        let result = save_prompt_in(root.path(), overrides.path(), "brain", "nao-existe.md", "texto");
        assert_eq!(result["ok"], false);
        assert!(result["error"].as_str().unwrap().contains("desconhecido"));
    }

    #[test]
    fn save_rejects_directory_traversal_even_with_a_real_default_nearby() {
        let root = Dir::new("save-root-traversal");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("save-overrides-traversal");

        let result = save_prompt_in(root.path(), overrides.path(), "brain", "../monologue.md", "texto malicioso");
        assert_eq!(result["ok"], false);
    }

    #[test]
    fn save_writes_override_and_reports_dropped_placeholders() {
        let root = Dir::new("save-root-write");
        write_file(root.path(), "brain/monologue.md", "Fala {{tema}} com {{emocao}}");
        let overrides = Dir::new("save-overrides-write");

        let result = save_prompt_in(root.path(), overrides.path(), "brain", "monologue.md", "Só fala {{tema}}");
        assert_eq!(result["ok"], true);
        assert_eq!(result["reset"], false);
        assert_eq!(result["droppedPlaceholders"], json!(["emocao"]));
        assert_eq!(
            fs::read_to_string(overrides.path().join("brain").join("monologue.md")).unwrap(),
            "Só fala {{tema}}"
        );
    }

    /// M7: o editor não grava um `system.md` sem o piso de segurança (o loader o ignoraria).
    #[test]
    fn save_refuses_a_system_prompt_without_the_safety_floor() {
        let root = Dir::new("save-root-floor");
        write_file(root.path(), "brain/system.md", "default\nSafety floor");
        let overrides = Dir::new("save-overrides-floor");

        let result = save_prompt_in(root.path(), overrides.path(), "brain", "system.md", "sem piso");
        assert_eq!(result["ok"], false);
        assert!(!overrides.path().join("brain").join("system.md").exists());
        assert_eq!(save_prompt_in(root.path(), overrides.path(), "brain", "system.md", "com piso\nSafety floor")["ok"], true);
    }

    #[test]
    fn save_with_blank_text_resets_instead_of_writing_an_empty_file() {
        let root = Dir::new("save-root-blank");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("save-overrides-blank");
        write_file(overrides.path(), "brain/monologue.md", "override antigo");

        let result = save_prompt_in(root.path(), overrides.path(), "brain", "monologue.md", "   ");
        assert_eq!(result["ok"], true);
        assert_eq!(result["reset"], true);
        assert!(!overrides.path().join("brain").join("monologue.md").exists());
    }

    #[test]
    fn save_rejects_text_over_the_char_limit() {
        let root = Dir::new("save-root-over-limit");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("save-overrides-over-limit");

        let huge = "a".repeat(PROMPT_OVERRIDE_MAX_CHARS + 1);
        let result = save_prompt_in(root.path(), overrides.path(), "brain", "monologue.md", &huge);
        assert_eq!(result["ok"], false);
        assert!(!overrides.path().join("brain").join("monologue.md").exists());
    }

    #[test]
    fn save_accepts_text_at_exactly_the_char_limit() {
        let root = Dir::new("save-root-at-limit");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("save-overrides-at-limit");

        let max = "a".repeat(PROMPT_OVERRIDE_MAX_CHARS);
        let result = save_prompt_in(root.path(), overrides.path(), "brain", "monologue.md", &max);
        assert_eq!(result["ok"], true);
    }

    #[test]
    fn reset_removes_only_the_override_file() {
        let root = Dir::new("reset-root");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("reset-overrides");
        write_file(overrides.path(), "brain/monologue.md", "override");

        let result = reset_prompt_in(root.path(), overrides.path(), "brain", "monologue.md");
        assert_eq!(result["ok"], true);
        assert!(!overrides.path().join("brain").join("monologue.md").exists());
        // O default empacotado nunca é tocado por um reset.
        assert!(root.path().join("brain").join("monologue.md").exists());
    }

    #[test]
    fn reset_of_unknown_prompt_is_rejected() {
        let root = Dir::new("reset-root-unknown");
        let overrides = Dir::new("reset-overrides-unknown");

        let result = reset_prompt_in(root.path(), overrides.path(), "brain", "nao-existe.md");
        assert_eq!(result["ok"], false);
    }

    #[test]
    fn reset_without_an_existing_override_is_a_harmless_noop() {
        let root = Dir::new("reset-root-noop");
        write_file(root.path(), "brain/monologue.md", "default");
        let overrides = Dir::new("reset-overrides-noop");

        let result = reset_prompt_in(root.path(), overrides.path(), "brain", "monologue.md");
        assert_eq!(result["ok"], true);
    }
}
