//! Gate estrutural para `specs/01-architecture.md` ("Ciclo de vida"): "todo timeout, interval,
//! listener ou processo que um módulo inicia é destruído quando o módulo desliga". `shutdown.rs`
//! deu o mecanismo (`shutdown::subscribe()`/`until_triggered`, disparado por `shutdown::trigger()`
//! no `RunEvent::ExitRequested` de `lib.rs`); este arquivo é a prova de que todo loop de fundo em
//! `modules/` realmente o usa, em vez de confiar em "todo mundo lembrou" — mesma classe de defeito
//! que `check:topology`/`nothing_the_boot_calls_uses_bare_tokio_spawn` (`modules/mod.rs`) já
//! cobrem para os invariantes deles.
//!
//! **O que conta como "loop de fundo".** Um `tokio::spawn`/`tauri::async_runtime::spawn` cujo
//! corpo contém `loop {` ou `while let` — as duas formas que este crate usa para "roda até alguém
//! mandar parar" (timers de tick e assinantes do `EventBus`, ver `shutdown.rs`). Um `spawn` sem
//! nenhum dos dois é, por definição, limitado (uma sequência de `.await` que termina sozinha —
//! ex.: `expression_action.rs::run_expression`, um `sleep` + reversão de estado, nunca mais que
//! `duration_ms` de vida) e não precisa de sinal de desligamento nenhum.
//!
//! **Como isola o corpo do `spawn`.** Contagem de chaves a partir do `{` que abre o bloco
//! `async move`, ignorando chaves dentro de string/char literais e comentários de linha — o
//! suficiente para código Rust real (sem literais multi-linha maliciosos), e é exatamente o motivo
//! de [`expression_action.rs`] não aparecer como falso positivo: sem essa contagem, um `grep`/janela
//! fixa vazaria para o `#[cfg(test)] mod tests` do MESMO arquivo, que usa `while let` em um helper
//! de teste sem relação nenhuma com o `spawn` de produção.
//!
//! **Alçada estreita, com dono.** `ALLOWLIST` é a única forma de silenciar este gate — vazia hoje,
//! porque toda ocorrência real já foi corrigida nesta rodada (ver `git log` desta mudança). Um novo
//! loop de fundo sem `shutdown::` reprova a suíte; se for uma exceção genuína, ela entra ali com o
//! motivo, nunca calada.

use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;

/// Caminho relativo a este arquivo (`(arquivo, número da linha do "loop de fundo" reportado pela
/// varredura, motivo)`. Vazia por padrão — ver o cabeçalho do módulo.
const ALLOWLIST: &[(&str, usize, &str)] = &[];

fn modules_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("modules")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Acha o `}` que fecha o `{` em `open_brace_index` (que já aponta para o próprio `{`), pulando
/// chaves dentro de string/char literais e comentários de linha — não um parser Rust completo, mas
/// o bastante para código real deste crate (nenhum `{`/`}` cru sobrevive dentro de uma string sem
/// estar entre aspas ou ser escapado).
fn matching_brace(source: &str, open_brace_index: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0i32;
    let mut i = open_brace_index;
    let mut in_string = false;
    let mut in_char = false;
    let mut in_line_comment = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if in_char {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'\'' {
                in_char = false;
            }
            i += 1;
            continue;
        }
        match b {
            b'/' if bytes.get(i + 1) == Some(&b'/') => in_line_comment = true,
            b'"' => in_string = true,
            b'\'' => in_char = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

struct Violation {
    file: String,
    line: usize,
}

fn scan_file(path: &Path, spawn_re: &Regex, violations: &mut Vec<Violation>) {
    let Ok(source) = fs::read_to_string(path) else { return };
    let relative = path
        .strip_prefix(modules_root().parent().expect("src/"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    for m in spawn_re.find_iter(&source) {
        // `m` termina no `{` de `async move {` — usa exatamente esse índice como abertura.
        let open = m.end() - 1;
        let Some(close) = matching_brace(&source, open) else { continue };
        let body = &source[open..=close];

        let is_background_loop = body.contains("loop {") || body.contains("while let");
        if !is_background_loop {
            continue;
        }
        if references_shutdown_in_code(body) {
            continue;
        }

        let line = source[..m.start()].matches('\n').count() + 1;
        if ALLOWLIST.iter().any(|(f, l, _)| *f == relative && *l == line) {
            continue;
        }
        violations.push(Violation { file: relative.clone(), line });
    }
}

/// `shutdown::` em CÓDIGO, não em comentário: um `// TODO: ligar shutdown::until_triggered` num
/// laço que nunca para passava como fiado (gate lendo prosa como código).
fn references_shutdown_in_code(body: &str) -> bool {
    body.lines().any(|line| line.split("//").next().unwrap_or_default().contains("shutdown::"))
}

fn scan_real_tree() -> Vec<Violation> {
    let spawn_re = Regex::new(r"(tokio::spawn|tauri::async_runtime::spawn)\s*\(\s*async move\s*\{")
        .expect("spawn_re válido");
    let mut files = Vec::new();
    collect_rs_files(&modules_root(), &mut files);
    files.sort();

    let mut violations = Vec::new();
    for file in files {
        // Este PRÓPRIO arquivo fica de fora: os testes abaixo embutem, como string literal, um
        // `spawn` de exemplo SEM `shutdown::` só para provar que o scanner acusaria um de verdade
        // — sem esta exclusão, o gate acusaria a si mesmo pela string, não por código de produção.
        if file.file_name().is_some_and(|name| name == "lifecycle_shutdown_gate.rs") {
            continue;
        }
        scan_file(&file, &spawn_re, &mut violations);
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A prova real: nenhum loop de fundo em `modules/` foge do sinal de desligamento — se este
    /// teste falhar, o `git blame` da linha reportada é quem introduziu o novo loop, e a correção é
    /// selecionar em `crate::shutdown::until_triggered` (ver o cabeçalho de `shutdown.rs`) ou, se
    /// for mesmo uma exceção, documentá-la em `ALLOWLIST` com o motivo.
    #[test]
    fn every_background_spawn_loop_selects_on_the_shutdown_signal() {
        let violations = scan_real_tree();
        assert!(
            violations.is_empty(),
            "loop(s) de fundo sem `crate::shutdown::until_triggered` no `select!` (ou entrada em \
             ALLOWLIST com motivo): {}",
            violations.iter().map(|v| format!("{}:{}", v.file, v.line)).collect::<Vec<_>>().join(", ")
        );
    }

    /// Sonda negativa: o mecanismo tem de ACUSAR um loop de fundo de verdade que não referencia o
    /// desligamento, senão a varredura acima passa mesmo lendo de menos. Ver
    /// `project_gates_com_piso_de_alcance` / `project_probe_gates_sem_tocar_fonte` (memória do
    /// projeto): probe roda em `TempDir`, nunca escreve dentro da árvore rastreada pelo git.
    #[test]
    fn probe_negative_catches_a_background_loop_without_the_shutdown_signal() {
        let source = r#"
pub fn spawn_something() {
    tauri::async_runtime::spawn(async move {
        loop {
            do_the_thing().await;
        }
    });
}
"#;
        let spawn_re = Regex::new(r"(tokio::spawn|tauri::async_runtime::spawn)\s*\(\s*async move\s*\{")
            .expect("spawn_re válido");
        let mut violations = Vec::new();
        // Reusa `scan_file` via um arquivo escrito num diretório fora da árvore rastreada.
        let scratch = std::env::temp_dir().join(format!(
            "mika-lifecycle-gate-probe-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("relógio")
                .as_nanos()
        ));
        fs::create_dir_all(&scratch).expect("dir de probe");
        let probe_file = scratch.join("probe.rs");
        fs::write(&probe_file, source).expect("escrever probe.rs");

        scan_file(&probe_file, &spawn_re, &mut violations);
        let result = fs::remove_dir_all(&scratch);

        assert_eq!(violations.len(), 1, "o probe negativo devia acusar exatamente 1 loop sem shutdown");
        result.expect("limpeza do diretório de probe");
    }

    /// Sonda positiva simétrica: o MESMO loop, com o `select!` correto, não deve ser acusado — sem
    /// isto, o teste acima provaria só que o gate acusa QUALQUER `spawn` com `loop {`, não que ele
    /// distingue quem já está correto.
    #[test]
    fn a_shutdown_mention_in_a_comment_does_not_count_as_wiring() {
        assert!(!references_shutdown_in_code("loop {\n    // TODO: ligar shutdown::until_triggered aqui\n    tick().await;\n}"));
        assert!(references_shutdown_in_code("tokio::select! { _ = shutdown::until_triggered(&mut rx) => break, }"));
    }

    #[test]
    fn a_background_loop_that_selects_on_shutdown_is_not_a_violation() {
        let source = r#"
pub fn spawn_something() {
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                _ = do_the_thing() => {}
            }
        }
    });
}
"#;
        let spawn_re = Regex::new(r"(tokio::spawn|tauri::async_runtime::spawn)\s*\(\s*async move\s*\{")
            .expect("spawn_re válido");
        let mut violations = Vec::new();
        let scratch = std::env::temp_dir().join(format!(
            "mika-lifecycle-gate-probe-ok-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("relógio")
                .as_nanos()
        ));
        fs::create_dir_all(&scratch).expect("dir de probe");
        let probe_file = scratch.join("probe.rs");
        fs::write(&probe_file, source).expect("escrever probe.rs");

        scan_file(&probe_file, &spawn_re, &mut violations);
        let result = fs::remove_dir_all(&scratch);

        assert!(violations.is_empty(), "loop já correto não deveria ser acusado: {:?}", violations.iter().map(|v| format!("{}:{}", v.file, v.line)).collect::<Vec<_>>());
        result.expect("limpeza do diretório de probe");
    }

    /// Prova de que a contagem de chaves não vaza para um `#[cfg(test)] mod tests` no MESMO
    /// arquivo — a razão de existir de `matching_brace` em vez de uma janela fixa (ver o cabeçalho
    /// do módulo, `expression_action.rs`).
    #[test]
    fn scan_does_not_leak_into_a_sibling_test_module_using_while_let() {
        let source = r#"
pub fn spawn_bounded_timer() {
    tokio::spawn(async move {
        do_the_once_off_thing().await;
    });
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn some_test_uses_while_let_but_is_unrelated() {
        while let Some(x) = something().await {
            handle(x);
        }
    }
}
"#;
        let spawn_re = Regex::new(r"(tokio::spawn|tauri::async_runtime::spawn)\s*\(\s*async move\s*\{")
            .expect("spawn_re válido");
        let mut violations = Vec::new();
        let scratch = std::env::temp_dir().join(format!(
            "mika-lifecycle-gate-probe-scope-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("relógio")
                .as_nanos()
        ));
        fs::create_dir_all(&scratch).expect("dir de probe");
        let probe_file = scratch.join("probe.rs");
        fs::write(&probe_file, source).expect("escrever probe.rs");

        scan_file(&probe_file, &spawn_re, &mut violations);
        let result = fs::remove_dir_all(&scratch);

        assert!(
            violations.is_empty(),
            "o `while let` do módulo de teste vizinho não pode contar como loop de fundo do `spawn` de produção"
        );
        result.expect("limpeza do diretório de probe");
    }
}
