//! Steamworks mínimo da build Steam (`--features steam`). Sem a feature este arquivo não compila.
//!
//! O que faz, e só isto:
//! - `relaunch_through_steam_if_needed`: exe aberto fora da Steam (duplo clique) é relançado pelo
//!   cliente, que é quem checa a licença e atualiza o binário (SteamPipe) — nunca um updater próprio.
//! - `start`: inicia a API, mantém o pump de callbacks e publica a Rich Presence.
//! - `unlock`: conquista por nome de API; os 5 marcos saem do barramento (`milestone_for`).
//!
//! Steam fechada, app sem licença ou conquista ausente no Steamworks: log e segue — a Mika nunca
//! deixa de abrir por causa disto.
//!
//! O AppID é o de `MIKA_STEAM_APP_ID` na COMPILAÇÃO (o CI injeta); em runtime a mesma variável de
//! ambiente vale para desenvolvimento. Sem AppID conhecido, não há relançamento e a inicialização
//! depende de `steam_appid.txt` ou de a Steam ter lançado o processo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use serde_json::Value;
use steamworks::{AppId, Client};
use tauri::AppHandle;

use crate::events::{bus, EventKey};

/// Nomes de API das conquistas — têm de ser criados IGUAIS em Steamworks (ver `steam/README.md`).
pub const ACH_FIRST_CONVERSATION: &str = "MIKA_FIRST_CONVERSATION";
pub const ACH_FIRST_VOICE: &str = "MIKA_FIRST_VOICE";
pub const ACH_SEVEN_DAYS: &str = "MIKA_SEVEN_DAYS";
pub const ACH_FIRST_MEMORY: &str = "MIKA_FIRST_MEMORY";
pub const ACH_FIRST_GAME_COMMENT: &str = "MIKA_FIRST_GAME_COMMENT";

const PRESENCE_TEXT: &str = "Conversando com a Mika";
const USAGE_FILE: &str = "steam-usage-days.json";
const DAYS_FOR_ACHIEVEMENT: usize = 7;

static CLIENT: OnceLock<Client> = OnceLock::new();
/// Por conquista: tentativas FALHAS até agora; `u8::MAX` = concluída (ver [`settle`]).
static PROGRESS: Mutex<Option<HashMap<&'static str, u8>>> = Mutex::new(None);

/// Quantas vezes uma conquista que a Steam recusa é tentada antes de desistir (cada chunk de chat
/// chama `unlock`: sem teto, uma conquista desconhecida no Steamworks logaria a cada chunk).
const MAX_FAILED_ATTEMPTS: u8 = 5;

fn app_id() -> Option<AppId> {
    let from_build = option_env!("MIKA_STEAM_APP_ID").map(str::to_owned);
    from_build
        .or_else(|| std::env::var("MIKA_STEAM_APP_ID").ok())
        .and_then(|raw| raw.trim().parse::<u32>().ok())
        .filter(|id| *id != 0)
        .map(AppId)
}

/// `true` quando o processo deve sair já: a Steam foi acionada para relançar o jogo. Primeira
/// linha de `run()` — antes de qualquer janela, plugin ou thread. Build de debug nunca relança
/// (`tauri dev` não é um jogo da biblioteca).
pub fn relaunch_through_steam_if_needed() -> bool {
    let Some(id) = app_id() else { return false };
    if cfg!(debug_assertions) || !steamworks::restart_app_if_necessary(id) {
        return false;
    }
    // `eprintln!` de propósito: roda ANTES do plugin de log (primeira linha de `run()`), e sem logger
    // registrado o crate `log` descarta a mensagem — com `log::info!` esta linha nunca seria vista.
    eprintln!("steam: aberto fora da Steam, relançando pelo cliente (AppID {})", id.0);
    true
}

/// Inicia a Steamworks. Chamado no `setup` do Tauri, quando o logger e o `app_data_dir` já existem.
pub fn start(app: &AppHandle) {
    let client = match Client::init() {
        Ok(client) => client,
        Err(error) => {
            log::info!("steam: API indisponível, seguindo sem integração ({error})");
            return;
        }
    };
    let friends = client.friends();
    // `steam_display` aponta o token `#Status` (`"#Status" "%status%"` no Steamworks) e `status` leva o texto.
    friends.set_rich_presence("steam_display", Some("#Status"));
    friends.set_rich_presence("status", Some(PRESENCE_TEXT));
    let _ = CLIENT.set(client.clone());

    std::thread::Builder::new()
        .name("steam-callbacks".into())
        .spawn(move || loop {
            client.run_callbacks();
            std::thread::sleep(Duration::from_millis(100));
        })
        .map_err(|error| log::warn!("steam: pump de callbacks não subiu ({error})"))
        .ok();

    let usage_file = crate::state::app_data_dir(app).ok().map(|dir| dir.join(USAGE_FILE));
    tauri::async_runtime::spawn(watch_milestones(usage_file));
    log::info!("steam: integração ativa");
}

/// Destrava uma conquista (idempotente). Sem Steam ativa ou com a conquista desconhecida só loga.
pub fn unlock(name: &'static str) {
    let Some(client) = CLIENT.get() else { return };
    let mut progress = PROGRESS.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    settle(progress.get_or_insert_with(HashMap::new), name, || try_unlock(client, name));
}

/// Roda `attempt` só se `name` ainda não foi concluída nem esgotou as tentativas. Concluída = a Steam
/// aceitou (ou a conquista já era do jogador): uma falha TRANSITÓRIA (stats ainda carregando, store
/// recusado) não vale para a sessão inteira, tenta de novo no próximo marco.
fn settle(state: &mut HashMap<&'static str, u8>, name: &'static str, attempt: impl FnOnce() -> bool) {
    let failed = state.entry(name).or_insert(0);
    if *failed >= MAX_FAILED_ATTEMPTS {
        return;
    }
    *failed = if attempt() { u8::MAX } else { *failed + 1 };
}

/// Uma tentativa na Steam. `true` = concluída (destravada agora ou já destravada antes).
fn try_unlock(client: &Client, name: &'static str) -> bool {
    let stats = client.user_stats();
    let achievement = stats.achievement(name);
    match achievement.get() {
        Ok(true) => return true,
        Ok(false) => {}
        Err(()) => {
            log::warn!("steam: conquista '{name}' desconhecida ou stats ainda não carregadas");
            return false;
        }
    }
    if achievement.set().is_err() || stats.store_stats().is_err() {
        log::warn!("steam: não consegui gravar a conquista '{name}'");
        return false;
    }
    log::info!("steam: conquista '{name}' destravada");
    true
}

/// Marco que o evento representa, se algum. Pura: o que faz de um evento do barramento uma conquista.
pub(crate) fn milestone_for(key: EventKey, payload: &Value) -> Option<&'static str> {
    match key {
        // Texto realmente streamado de uma resposta: erro e cancelamento não chegam aqui.
        EventKey::StreamChunk => payload
            .get("text")
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
            .then_some(ACH_FIRST_CONVERSATION),
        EventKey::SpeechStarted => Some(ACH_FIRST_VOICE),
        // A Mika guardou algo sobre uma pessoa (ficha criada/atualizada ou menção nova).
        EventKey::PersonUpdated | EventKey::PersonMentionAdded => Some(ACH_FIRST_MEMORY),
        // `origin` é posto por `gamer::game_moment_commentary` ao publicar o comentário.
        EventKey::SpeechRequest => {
            (payload.get("origin").and_then(Value::as_str) == Some("game-moment")).then_some(ACH_FIRST_GAME_COMMENT)
        }
        _ => None,
    }
}

/// Registra `today` entre os dias de uso. `true` quando o dia é novo.
pub(crate) fn record_day(days: &mut Vec<String>, today: &str) -> bool {
    if days.iter().any(|day| day == today) {
        return false;
    }
    days.push(today.to_owned());
    true
}

fn load_days(path: &Path) -> Vec<String> {
    std::fs::read(path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

/// Um dia de uso é um dia com pelo menos uma conversa (`ThinkingStart`). Guardado em disco só até
/// a conquista sair; depois disso a Steam é a fonte de verdade.
fn track_usage_day(file: &Option<PathBuf>, days: &mut Vec<String>) {
    let Some(path) = file else { return };
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    if !record_day(days, &today) {
        return;
    }
    if let Err(error) = crate::config::atomic::write_atomic(path, &serde_json::to_vec(days).unwrap_or_default()) {
        log::warn!("steam: não gravei os dias de uso ({error})");
    }
    if days.len() >= DAYS_FOR_ACHIEVEMENT {
        unlock(ACH_SEVEN_DAYS);
    }
}

async fn watch_milestones(usage_file: Option<PathBuf>) {
    let mut days = usage_file.as_deref().map(load_days).unwrap_or_default();
    let mut subscription = bus().subscribe();
    while let Some(envelope) = subscription.recv().await {
        if let Some(name) = milestone_for(envelope.key, &envelope.payload) {
            unlock(name);
        }
        if envelope.key == EventKey::ThinkingStart {
            track_usage_day(&usage_file, &mut days);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn each_real_event_maps_to_its_own_milestone() {
        assert_eq!(milestone_for(EventKey::StreamChunk, &json!({ "text": "oi" })), Some(ACH_FIRST_CONVERSATION));
        assert_eq!(milestone_for(EventKey::SpeechStarted, &json!({})), Some(ACH_FIRST_VOICE));
        assert_eq!(milestone_for(EventKey::PersonUpdated, &json!({ "personId": "ada" })), Some(ACH_FIRST_MEMORY));
        assert_eq!(
            milestone_for(EventKey::SpeechRequest, &json!({ "text": "boa!", "origin": "game-moment" })),
            Some(ACH_FIRST_GAME_COMMENT)
        );
    }

    #[test]
    fn empty_chunks_and_ordinary_speech_requests_unlock_nothing() {
        assert_eq!(milestone_for(EventKey::StreamChunk, &json!({ "text": "  " })), None);
        assert_eq!(milestone_for(EventKey::SpeechRequest, &json!({ "text": "oi" })), None);
        assert_eq!(milestone_for(EventKey::ThinkingStop, &json!({})), None);
    }

    #[test]
    fn a_repeated_day_is_not_counted_twice() {
        let mut days = Vec::new();
        assert!(record_day(&mut days, "2026-10-01"));
        assert!(!record_day(&mut days, "2026-10-01"));
        assert!(record_day(&mut days, "2026-10-02"));
        assert_eq!(days.len(), 2);
    }

    /// A conquista só conta como concluída depois de a Steam aceitar: marcar antes fazia uma falha
    /// transitória (stats ainda carregando) valer para a sessão inteira. E uma conquista que nunca dá
    /// certo (desconhecida no Steamworks) desiste depois de algumas tentativas, em vez de logar a cada chunk.
    #[test]
    fn a_failed_unlock_is_retried_and_only_a_success_settles_it() {
        let mut state = std::collections::HashMap::new();
        let mut calls = 0;
        settle(&mut state, ACH_FIRST_VOICE, || {
            calls += 1;
            false
        });
        settle(&mut state, ACH_FIRST_VOICE, || {
            calls += 1;
            true
        });
        settle(&mut state, ACH_FIRST_VOICE, || {
            calls += 1;
            true
        });
        assert_eq!(calls, 2, "falhou, tentou de novo, deu certo e parou");

        let mut forever_failing = 0;
        for _ in 0..50 {
            settle(&mut state, ACH_SEVEN_DAYS, || {
                forever_failing += 1;
                false
            });
        }
        assert!(forever_failing < 50, "tem de desistir de uma conquista que nunca dá certo ({forever_failing} tentativas)");
    }

    /// `steam-usage-days.json` guarda o progresso da conquista de 7 dias: gravação truncada por um
    /// crash o zerava. Com escrita atômica uma falha deixa o arquivo ANTIGO intacto.
    #[test]
    fn a_failed_usage_write_leaves_the_previous_file_intact() {
        let dir = std::env::temp_dir().join(format!("mika-steam-usage-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(USAGE_FILE);
        std::fs::write(&file, r#"["2026-10-01"]"#).unwrap();
        // diretório no lugar do temporário: a escrita atômica falha de forma determinística
        std::fs::create_dir(dir.join(format!("{USAGE_FILE}.tmp"))).unwrap();

        track_usage_day(&Some(file.clone()), &mut vec!["2026-10-01".to_string()]);

        assert_eq!(std::fs::read_to_string(&file).unwrap(), r#"["2026-10-01"]"#);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
