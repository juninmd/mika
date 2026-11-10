//! `mika:chat-report` — o botão "Reportar esta resposta" do histórico. Grava `reports/<ts>.json`
//! NO COMPUTADOR do usuário: o texto da resposta, o modelo que a gerou e a persona. Nunca tela,
//! áudio nem a fala do usuário, e nada é enviado a lugar nenhum — o relatório é o que o usuário
//! mostra a quem quiser (suporte, plataforma). O botão de e-mail não existe enquanto o endereço de
//! suporte for placeholder em `EULA.md`/`PRIVACY.md`; `reports/` entra em exportar e apagar
//! (`features::privacy`).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

pub(super) const CHANNEL: &str = "mika:chat-report";
/// Pasta sob `<app_data>`; `features::privacy` varre a MESMA (ver `clear_ext.rs`/`export.rs`).
pub(crate) const REPORTS_DIR: &str = "reports";
/// Espelho de `desktopChatReportRequestSchema` (`desktop-ipc-core-schemas.ts`).
const MAX_CONTENT_CHARS: usize = 20_000;

fn parse_content(payload: &Value) -> Result<String, String> {
    let content = payload.get("content").and_then(Value::as_str).map(str::trim).unwrap_or_default();
    if content.is_empty() {
        return Err(format!("{CHANNEL} exige 'content' texto não vazio"));
    }
    if content.chars().count() > MAX_CONTENT_CHARS {
        return Err(format!("{CHANNEL}: 'content' passa de {MAX_CONTENT_CHARS} caracteres"));
    }
    Ok(content.to_string())
}

/// O conteúdo do relatório: a resposta, o par provedor/modelo e a persona vigente. Nenhum campo vem
/// do renderer além de `content`; nada de tela, áudio ou histórico.
fn build_report(store: &ConfigStore, content: &str, created_at_ms: u64) -> Value {
    let (provider, model) = super::send::resolve_health_key(store);
    let text = |path: &str| store.get(path).and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
    json!({
        "createdAt": created_at_ms,
        "appVersion": env!("CARGO_PKG_VERSION"),
        "response": content,
        "model": { "provider": provider, "model": model },
        "persona": {
            "name": text("character.name"),
            "personalityProfile": text("character.personalityProfile"),
            "relationshipLevel": super::effective_relationship_level(store),
        },
    })
}

/// `<dir>/<ts>.json`, escrita atômica; devolve o caminho. Dois relatórios no mesmo milissegundo não
/// se sobrescrevem (sufixo `-n`).
fn write_report(dir: &Path, created_at_ms: u64, report: &Value) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("não criou {}: {e}", dir.display()))?;
    let mut path = dir.join(format!("{created_at_ms}.json"));
    for attempt in 1.. {
        if !path.exists() {
            break;
        }
        path = dir.join(format!("{created_at_ms}-{attempt}.json"));
    }
    let body = serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?;
    crate::config::atomic::write_atomic(&path, &body).map_err(|e| format!("não gravou {}: {e}", path.display()))?;
    Ok(path)
}

pub(super) async fn handle(app: &AppHandle, ctx: &InvokeContext, payload: Value) -> Result<Value, String> {
    if !is_owner_surface(ctx.window_type) {
        return Err(format!("{CHANNEL} recusado para {:?}", ctx.window_type));
    }
    let content = parse_content(&payload)?;
    let store = crate::state::config_store(app)?;
    let data_dir = crate::state::app_data_dir(app).map_err(|e| e.to_string())?;
    let now = super::send::now_millis();
    let path = write_report(&data_dir.join(REPORTS_DIR), now, &build_report(&store, &content, now))?;
    Ok(json!({ "file": path.display().to_string() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mika-chat-report-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn store(dir: &Path) -> ConfigStore {
        ConfigStore::open(dir, crate::config::schemas::registry()).unwrap()
    }

    #[test]
    fn only_a_non_empty_bounded_response_text_is_accepted() {
        assert_eq!(parse_content(&json!({ "content": "  oi  " })).unwrap(), "oi");
        for bad in [json!({}), json!({ "content": "" }), json!({ "content": "   " }), json!({ "content": 7 })] {
            assert!(parse_content(&bad).is_err(), "{bad}");
        }
        assert!(parse_content(&json!({ "content": "x".repeat(MAX_CONTENT_CHARS + 1) })).is_err());
    }

    /// O relatório carrega a resposta, o modelo e a persona — e nenhum campo de tela, áudio ou fala
    /// do usuário, nem mesmo quando o config tem chave de API e perfil preenchidos.
    #[test]
    fn the_report_has_the_response_model_and_persona_and_nothing_else() {
        let dir = temp_dir("build");
        let store = store(&dir);
        store.set("llm.provider", json!("openrouter")).unwrap();
        store.set("user.name", json!("Ana")).unwrap();
        let report = build_report(&store, "resposta ruim", 1_700_000_000_000);

        assert_eq!(report["response"], json!("resposta ruim"));
        assert_eq!(report["model"]["provider"], json!("openrouter"));
        assert_eq!(report["persona"]["relationshipLevel"], json!("friend"));
        // Ordenado: com `serde_json/preserve_order` (ligado pela `rig-core` ≥ 0.43) as chaves saem na
        // ordem de inserção, e o que este teste afirma é o CONJUNTO ("nada além disto").
        let mut keys: Vec<&str> = report.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["appVersion", "createdAt", "model", "persona", "response"]);
        assert!(!report.to_string().contains("Ana"), "o perfil do usuário não pode entrar no relatório");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn writing_creates_the_folder_and_never_overwrites_a_report_from_the_same_millisecond() {
        let dir = temp_dir("write").join("reports");
        let first = write_report(&dir, 42, &json!({ "response": "a" })).unwrap();
        let second = write_report(&dir, 42, &json!({ "response": "b" })).unwrap();
        assert_eq!(first, dir.join("42.json"));
        assert_eq!(second, dir.join("42-1.json"));
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(&first).unwrap()).unwrap()["response"], json!("a"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// Contrato cross-language: canal e limite são os do contrato TypeScript.
    #[test]
    fn the_channel_and_the_size_limit_match_the_typescript_contract() {
        let channels = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        let schemas = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-core-schemas.ts");
        assert!(channels.contains(&format!("ChatReport: '{CHANNEL}'")));
        assert!(schemas.contains(&format!("content: z.string().min(1).max({MAX_CONTENT_CHARS})")));
    }
}
