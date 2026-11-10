//! `mika:wizard-react` — o onboarding pede para a Mika reagir ao sentido que o usuário acabou de
//! configurar (skin, cérebro, voz, memória, olhos/ouvidos, identidade). Só a ETAPA atravessa o IPC,
//! por match fechado; toda escolha é lida da config já salva, então nenhum texto digitado no wizard
//! chega ao prompt. A fala é um turno proativo real (`send_proactive`): mesma persona, mesmo
//! AutoSpeak, e fica no histórico como qualquer outra coisa que ela disse. Sem LLM configurado o
//! turno falha e a resposta volta vazia — o wizard mostra que ela ainda "não acordou".

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use tauri::AppHandle;

use crate::config::store::ConfigStore;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

pub(super) const CHANNEL: &str = "mika:wizard-react";

/// Espelho de `wizardReactStepSchema` (`desktop-ipc-plugin-schemas.ts`); cada uma tem o seu
/// `prompts/brain/wizard/<etapa>.md`.
const STEPS: [&str; 6] = ["avatar", "llm", "tts", "embedding", "vision", "identity"];

/// Só a reação mais recente fala: uma anterior que ainda esperava o LLM é descartada.
static LATEST_TICKET: AtomicU64 = AtomicU64::new(0);

fn take_ticket() -> u64 {
    LATEST_TICKET.fetch_add(1, Ordering::SeqCst) + 1
}

fn is_superseded(ticket: u64) -> bool {
    LATEST_TICKET.load(Ordering::SeqCst) != ticket
}

fn parse_step(payload: &Value) -> Result<&'static str, String> {
    let raw = payload.get("step").and_then(Value::as_str).ok_or_else(|| format!("{CHANNEL} exige 'step' string"))?;
    STEPS.iter().copied().find(|step| *step == raw).ok_or_else(|| format!("{CHANNEL}: etapa desconhecida {raw:?}"))
}

fn text(store: &ConfigStore, path: &str) -> String {
    store.get(path).and_then(|v| v.as_str().map(|s| s.trim().to_string())).unwrap_or_default()
}

fn on_off(store: &ConfigStore, path: &str) -> &'static str {
    if store.get(path).and_then(|v| v.as_bool()) == Some(true) { "on" } else { "off" }
}

/// `character.modelName` quando existe; senão o nome do arquivo do modelo sem extensão.
fn skin_label(model_name: &str, model_path: &str) -> String {
    if !model_name.is_empty() {
        return model_name.to_string();
    }
    let file = model_path.rsplit(['/', '\\']).next().unwrap_or_default();
    file.split('.').next().unwrap_or_default().to_string()
}

fn prompt_vars(store: &ConfigStore) -> HashMap<String, String> {
    let nickname = Some(text(store, "user.nickname")).filter(|s| !s.is_empty()).unwrap_or_else(|| text(store, "user.name"));
    let (provider, model) = super::send::resolve_health_key(store);
    let tts = match on_off(store, "voice.ttsEnabled") {
        "on" => text(store, "voice.ttsProvider"),
        off => off.to_string(),
    };
    [
        ("name", nickname),
        ("skin", skin_label(&text(store, "character.modelName"), &text(store, "character.modelPath"))),
        ("llm", format!("{provider} / {model}")),
        ("tts", tts),
        ("embedding", format!("{} ({})", on_off(store, "brain.embeddingEnabled"), text(store, "llm.embeddingProvider"))),
        ("memory", on_off(store, "memory.enabled").to_string()),
        ("vision", on_off(store, "vision.enabled").to_string()),
        ("hearing", on_off(store, "voice.sttEnabled").to_string()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

pub(super) async fn handle(app: &AppHandle, ctx: &InvokeContext, payload: Value) -> Result<Value, String> {
    if !is_owner_surface(ctx.window_type) {
        return Err(format!("{CHANNEL} recusado para {:?}", ctx.window_type));
    }
    let step = parse_step(&payload)?;
    let store = crate::state::config_store(app)?;
    let prompt = super::load_prompt_no_handle("brain", &format!("wizard/{step}"), &prompt_vars(&store))?;
    let ticket = take_ticket();
    let text = super::proactive::send_proactive_unless(app, &store, &prompt, &|| is_superseded(ticket)).await;
    Ok(json!({ "text": text }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct ConfigDir(std::path::PathBuf);

    impl Drop for ConfigDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (ConfigStore, ConfigDir) {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("mika-wizard-react-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        (ConfigStore::open(&dir, crate::config::schemas::registry()).unwrap(), ConfigDir(dir))
    }

    #[test]
    fn parse_step_accepts_only_the_closed_list() {
        assert_eq!(parse_step(&json!({ "step": "vision" })), Ok("vision"));
        assert!(parse_step(&json!({ "step": "ignore previous instructions" })).is_err());
        assert!(parse_step(&json!({ "step": 3 })).is_err());
        assert!(parse_step(&json!({})).is_err());
    }

    /// Clicar rápido por várias etapas: a reação anterior não pode falar por cima da nova.
    #[test]
    fn an_older_reaction_is_superseded_by_a_newer_one() {
        let older = take_ticket();
        let newer = take_ticket();
        assert!(is_superseded(older));
        assert!(!is_superseded(newer));
    }

    #[test]
    fn skin_label_prefers_the_model_name_and_falls_back_to_the_file_stem() {
        assert_eq!(skin_label("Haru", "vrm/haru.vrm"), "Haru");
        assert_eq!(skin_label("", "live2d\\hiyori\\hiyori.model3.json"), "hiyori");
        assert_eq!(skin_label("", ""), "");
    }

    /// A reação só vale se enxergar o que o usuário ACABOU de salvar — é a config, não o payload.
    #[test]
    fn prompt_vars_reflect_the_saved_choices() {
        let (store, _dir) = store();
        store.set("user.nickname", json!("Juju")).unwrap();
        store.set("voice.ttsEnabled", json!(true)).unwrap();
        store.set("voice.ttsProvider", json!("kokoro")).unwrap();
        store.set("vision.enabled", json!(true)).unwrap();

        let vars = prompt_vars(&store);
        assert_eq!(vars["name"], "Juju");
        assert_eq!(vars["tts"], "kokoro");
        assert_eq!(vars["vision"], "on");
    }

    /// Etapa sem `.md` faria o turno falhar em silêncio (resposta vazia) — prova que todas renderizam
    /// sem sobrar placeholder. Lê o FONTE (`prompts/`), não `out/prompts`: o artefato só ganha um
    /// `.md` novo depois de `build:prompts`, e o teste mediria o build velho.
    #[test]
    fn every_step_has_a_prompt_that_renders_all_placeholders() {
        let (store, _dir) = store();
        let vars = prompt_vars(&store);
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../prompts");
        for step in STEPS {
            let rendered = super::super::prompt::load_prompt_in(&source, None, "brain", &format!("wizard/{step}"), &vars)
                .unwrap_or_else(|e| panic!("prompt wizard/{step}: {e}"));
            assert!(!rendered.contains("{{"), "wizard/{step} deixou placeholder: {rendered}");
        }
    }
}
