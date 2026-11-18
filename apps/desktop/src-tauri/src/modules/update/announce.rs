//! Mika fala da versão nova ao achá-la, e pergunta se pode atualizar — o "sim" é o card
//! `UpdatePrompt.tsx` (Atualizar agora / Depois). Antes só um toast genérico avisava, e o badge do
//! dock passava despercebido.
//!
//! Uma fala por versão por execução: o laço de 6 h e o check do boot/Configurações acham a MESMA
//! versão várias vezes, e repetir a pergunta a cada check viraria cobrança.

use std::collections::HashMap;
use std::sync::Mutex;

use tauri::AppHandle;

/// Changelog cortado antes do prompt: a release pode publicar páginas, e a fala tem 1-2 frases.
const NOTES_MAX_CHARS: usize = 600;

static ANNOUNCED: Mutex<Option<String>> = Mutex::new(None);

/// `true` só na primeira vez que `version` aparece desde a última versão anunciada.
fn first_sighting(announced: &Mutex<Option<String>>, version: &str) -> bool {
    let mut last = announced.lock().unwrap_or_else(|e| e.into_inner());
    if last.as_deref() == Some(version) {
        return false;
    }
    *last = Some(version.to_owned());
    true
}

fn notes_line(notes: Option<&str>) -> String {
    match notes.map(str::trim).filter(|n| !n.is_empty()) {
        Some(notes) => {
            let cut: String = notes.chars().take(NOTES_MAX_CHARS).collect();
            format!("Release notes (for context only, mention at most one highlight): {cut}")
        }
        None => String::new(),
    }
}

fn prompt_vars(version: &str, notes: Option<&str>, locale: &str) -> HashMap<String, String> {
    [("version", version.to_owned()), ("notesLine", notes_line(notes)), ("locale", crate::config::locale::language_name(locale).to_owned())]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect()
}

/// Chamado a cada check que achou versão; fala em segundo plano para não segurar a resposta do check.
pub(super) fn on_found(app: &AppHandle, version: &str, notes: Option<&str>) {
    if !first_sighting(&ANNOUNCED, version) {
        return;
    }
    let (app, version, notes) = (app.clone(), version.to_owned(), notes.map(str::to_owned));
    tauri::async_runtime::spawn(async move { speak(&app, &version, notes.as_deref()).await });
}

async fn speak(app: &AppHandle, version: &str, notes: Option<&str>) {
    let config = match crate::state::config_store(app) {
        Ok(config) => config,
        Err(error) => return log::error!("versão nova sem config para avisar: {error}"),
    };
    let vars = prompt_vars(version, notes, &crate::config::locale::app_locale(&config));
    match crate::modules::chat::load_prompt_no_handle("update", "new-version-speech", &vars) {
        Ok(prompt) if crate::modules::chat::proactive::send_reminder(app, &config, &prompt).await.is_empty() => {
            log::warn!("versão nova {version} avisada só pelo card: o turno proativo não gerou texto");
        }
        Ok(_) => log::info!("versão nova {version} anunciada"),
        Err(error) => log::error!("versão nova {version} sem fala: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asks_once_per_version_and_again_for_a_newer_one() {
        let announced = Mutex::new(None);
        assert!(first_sighting(&announced, "1.2.0"));
        assert!(!first_sighting(&announced, "1.2.0"), "o check de 6 h não pode repetir a pergunta");
        assert!(first_sighting(&announced, "1.3.0"));
    }

    #[test]
    fn notes_are_optional_and_bounded() {
        assert_eq!(notes_line(None), "");
        assert_eq!(notes_line(Some("  \n")), "");
        let long = "á".repeat(NOTES_MAX_CHARS * 2);
        assert!(notes_line(Some(&long)).chars().count() < NOTES_MAX_CHARS + 100);
    }
}
