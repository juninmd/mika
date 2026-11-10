//! `action=window` — port de `listOpenWindows`
//! (`apps/desktop/src/main/infrastructure/os/window-process-list.ts`), via `xcap` (já dependência
//! do crate — `capture.rs` já a usa para captura de tela/janela). **Refuta a premissa do cabeçalho
//! antigo de `mod.rs`**: a parte "precisa de shell-out" nunca foi verdade para esta ação — o TS
//! só usa shell-out (`powershell -EncodedCommand`) porque o Node não tem API nativa de enumeração
//! de janela; o Rust já tem uma via `xcap`, sem processo filho nenhum, e — mesma melhoria já
//! documentada em `process.rs` — funciona nos três SOs, não só Windows.
//!
//! A ÚNICA barreira real era a redação de PII do título (dois dos cinco padrões usam look-behind,
//! que a crate `regex` não suporta) — e essa barreira também já caiu: `redact_pii`
//! (`features::person_catalog::pii`) já porta os cinco padrões SEM look-behind (filtro pós-match
//! em vez de parte do regex), com teste de paridade contra o TS. Reaproveitado aqui, não duplicado.

use std::collections::HashSet;

use serde_json::{json, Value};
use xcap::Window;

use crate::modules::features::person_catalog::pii::redact_pii;

/// Espelha `Select-Object -First 40` do script PowerShell original (`INVENTORY_SCRIPT`).
const MAX_WINDOWS: usize = 40;

/// Núcleo puro, testável sem sessão gráfica: dado o inventário cru `(title, processName, pid)`,
/// filtra título vazio, deduplica por `pid` (uma janela por processo — mesmo recorte que
/// `MainWindowTitle` do `Get-Process` original já fazia, uma linha por processo) e redige PII do
/// título ANTES de sair. Ordem de entrada é preservada — `xcap::Window::all()` já devolve as
/// janelas ordenadas por coordenada Z (a mais em primeiro plano primeiro), então "primeira por
/// pid" já é a janela principal daquele processo.
fn build_windows_response(raw: Vec<(String, String, u32)>) -> Value {
    let mut seen_pids: HashSet<u32> = HashSet::new();
    let mut list: Vec<Value> = Vec::new();

    for (title, process_name, pid) in raw {
        if list.len() >= MAX_WINDOWS {
            break;
        }
        let title = title.trim();
        if title.is_empty() {
            continue;
        }
        if !seen_pids.insert(pid) {
            continue;
        }
        list.push(json!({ "title": redact_pii(title), "processName": process_name, "pid": pid }));
    }

    json!({ "success": true, "data": { "windows": list, "count": list.len() } })
}

/// `[{ title, processName, pid }]` — mesmo formato de `OpenWindow` (`window-process-list.ts`).
/// Falha de enumeração (sem sessão gráfica, permissão negada) vira erro NOMEADO, nunca lista vazia
/// silenciosa — mesmo contrato que `listOpenWindows` (`throw new Error(result.error)`) já tinha.
pub(super) fn list_open_windows() -> Value {
    match Window::all() {
        Ok(windows) => {
            let raw = windows
                .into_iter()
                .filter_map(|window| {
                    let title = window.title().ok()?;
                    let pid = window.pid().ok()?;
                    let process_name = window.app_name().unwrap_or_default();
                    Some((title, process_name, pid))
                })
                .collect();
            build_windows_response(raw)
        }
        Err(error) => json!({
            "success": false,
            "error": format!("inventário de janelas falhou: {error}"),
            "errorCode": "internal",
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mesmo caso do teste TS ("redige PII do título antes de devolver — a lista de janelas vai
    /// para o prompt do LLM", `desktop-tools.test.ts`) — prova que a redação está de fato
    /// costurada, sem depender de sessão gráfica real.
    #[test]
    fn redacts_pii_from_the_title_before_it_reaches_the_llm() {
        let response = build_windows_response(vec![("joao.silva@example.com — Gmail".into(), "chrome".into(), 42)]);
        let windows = response["data"]["windows"].as_array().unwrap();
        assert_eq!(windows[0]["title"], json!("[EMAIL] — Gmail"));
        assert_eq!(windows[0]["processName"], json!("chrome"));
        assert_eq!(windows[0]["pid"], json!(42));
    }

    #[test]
    fn empty_list_is_success_not_error() {
        let response = build_windows_response(vec![]);
        assert_eq!(response["success"], json!(true));
        assert_eq!(response["data"]["count"], json!(0));
    }

    #[test]
    fn windows_without_a_title_are_dropped() {
        let response = build_windows_response(vec![("".into(), "svchost".into(), 4), ("   ".into(), "svchost".into(), 5)]);
        assert_eq!(response["data"]["count"], json!(0));
    }

    /// NEGATIVO — dois HWNDs do MESMO processo (popup + janela principal) viram UMA entrada, a
    /// primeira em ordem de Z (mesmo recorte de `MainWindowTitle`, uma linha por processo).
    #[test]
    fn keeps_only_the_first_window_per_pid() {
        let response = build_windows_response(vec![
            ("Janela principal".into(), "code".into(), 900),
            ("Popup escondido".into(), "code".into(), 900),
        ]);
        let windows = response["data"]["windows"].as_array().unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(windows[0]["title"], json!("Janela principal"));
    }

    #[test]
    fn caps_at_forty_windows() {
        let raw: Vec<(String, String, u32)> = (0..50).map(|i| (format!("Janela {i}"), "app".into(), i as u32)).collect();
        let response = build_windows_response(raw);
        assert_eq!(response["data"]["count"], json!(MAX_WINDOWS));
    }

    /// Smoke test com o ambiente REAL — sem sessão gráfica (CI headless) a enumeração pode falhar,
    /// e a tool tem de devolver erro nomeado, nunca pânico; com sessão (dev, a maioria do CI deste
    /// repo) devolve uma lista de verdade. As duas formas são aceitáveis; o que este teste prova é
    /// que `Window::all()` está de fato ligado, não mockado.
    #[test]
    fn list_open_windows_dispatches_against_the_real_environment() {
        let result = list_open_windows();
        if result["success"] == json!(true) {
            assert!(result["data"]["windows"].as_array().unwrap().len() <= MAX_WINDOWS);
        } else {
            assert_eq!(result["errorCode"], json!("internal"));
        }
    }
}
