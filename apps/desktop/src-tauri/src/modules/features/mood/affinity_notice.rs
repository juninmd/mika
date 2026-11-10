//! O consumidor de `EventKey::MoodAffinityLevelUp` — porte de `features/mood/index.ts` (o
//! `ctx.events.on(Events.MOOD_AFFINITY_LEVEL_UP, …)` que vira `Events.NOTIFICATION`).
//!
//! Subir de nível de afinidade é das poucas coisas que a Mika conquista com o tempo, e o único
//! sinal disso para o usuário é o aviso na HUD. A chave era publicada por `persistence.rs` desde
//! que existe e **ninguém a lia**: o nível subia, o store gravava, e a tela nunca contava. Nada
//! falhava — o teste do publicador prova que publica, e consumidor inexistente não tem teste.
//!
//! A mensagem é montada AQUI, traduzida, e publicada como `EventKey::Notification`, que
//! `modules::hud_bridge` encaminha ao renderer no canal `hud:notification` — o mesmo que
//! `subscribeNotices` já escuta. Duas pontes em vez de uma direta porque `Notification` é o
//! formato genérico de aviso: qualquer outro módulo que precise avisar algo publica a mesma chave
//! em vez de inventar um canal.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};

/// Espelha `AFFINITY_LEVEL_LABEL_KEYS`. Rótulo desconhecido não inventa chave de tradução — cai no
/// próprio rótulo cru, que é melhor que um aviso com `mood.affinity.level.???` na cara do usuário.
fn label_key(label: &str) -> Option<&'static str> {
    match label {
        "new" => Some("mood.affinity.level.new"),
        "known" => Some("mood.affinity.level.known"),
        "friend" => Some("mood.affinity.level.friend"),
        "fan" => Some("mood.affinity.level.fan"),
        "bestie" => Some("mood.affinity.level.bestie"),
        _ => None,
    }
}

/// O aviso pronto, ou `None` quando o payload não descreve uma subida de nível — mesma guarda do
/// TS (`typeof data?.level !== 'number' || !data.label`). Pura: o laço abaixo só publica o que
/// esta função devolver, e é ela que os testes exercitam.
fn notice_from(payload: &Value, locale: &str) -> Option<Value> {
    let level = payload.get("level").and_then(Value::as_i64)?;
    let label = payload.get("label").and_then(Value::as_str)?;
    if label.is_empty() {
        return None;
    }

    let t = |key: &str| crate::modules::settings::t(key, locale);
    let label_text = label_key(label).map_or_else(|| label.to_string(), |key| t(key));

    Some(json!({
        "type": "success",
        "source": t("mood.affinity.notification.source"),
        "message": format!(
            "{} {} {} \"{}\"!",
            t("mood.affinity.notification.levelUpPrefix"),
            level,
            t("mood.affinity.notification.levelUpMiddle"),
            label_text
        ),
    }))
}

/// Assina o barramento e transforma cada subida de nível num aviso traduzido.
///
/// O locale é lido do `ConfigStore` a CADA evento, não capturado uma vez: trocar o idioma nas
/// configurações tem de valer para o próximo aviso, não só depois de reiniciar (mesma razão de
/// `tray::refresh_labels` existir).
pub fn spawn_affinity_notice_listener(store: std::sync::Arc<ConfigStore>) {
    // Assina ANTES do spawn — `broadcast` não entrega retroativamente.
    let mut sub = bus().subscribe();
    tauri::async_runtime::spawn(async move {
        let mut shutdown_rx = crate::shutdown::subscribe();
        loop {
            let envelope = tokio::select! {
                _ = crate::shutdown::until_triggered(&mut shutdown_rx) => break,
                received = sub.recv() => match received {
                    Some(envelope) => envelope,
                    None => break,
                },
            };
            if envelope.key != EventKey::MoodAffinityLevelUp {
                continue;
            }
            let locale = crate::config::locale::app_locale(&store);
            if let Some(notice) = notice_from(&envelope.payload, &locale) {
                bus().publish(EventKey::Notification, notice);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_up_becomes_a_translated_notice() {
        let notice = notice_from(&json!({ "level": 3, "label": "friend", "score": 42 }), "pt-BR")
            .expect("payload completo vira aviso");
        assert_eq!(notice["type"], "success");
        assert_eq!(notice["source"], "Afinidade");
        let message = notice["message"].as_str().expect("mensagem é texto");
        assert!(message.contains('3'), "o nível entra na mensagem: {message}");
        assert!(message.contains("Amiga"), "o rótulo entra traduzido: {message}");
    }

    /// O mesmo evento em outro idioma não pode sair em português — o aviso é a única superfície
    /// onde a afinidade aparece, e sair fixo num idioma é o defeito que a bandeja já teve.
    #[test]
    fn the_locale_decides_the_words() {
        let notice = notice_from(&json!({ "level": 2, "label": "known" }), "en-US").expect("aviso");
        assert_eq!(notice["source"], "Affinity");
        // "Acquaintance", não "Known": o rótulo sai do dicionário, não do nome interno da chave.
        assert!(notice["message"].as_str().expect("texto").contains("Acquaintance"));
    }

    #[test]
    fn an_incomplete_payload_never_becomes_a_notice() {
        assert!(notice_from(&json!({ "label": "friend" }), "pt-BR").is_none());
        assert!(notice_from(&json!({ "level": 3 }), "pt-BR").is_none());
        assert!(notice_from(&json!({ "level": 3, "label": "" }), "pt-BR").is_none());
    }

    /// Rótulo fora do catálogo usa o texto cru em vez de vazar a chave de tradução na tela.
    #[test]
    fn an_unknown_label_falls_back_to_itself() {
        let notice = notice_from(&json!({ "level": 9, "label": "soulmate" }), "pt-BR").expect("aviso");
        let message = notice["message"].as_str().expect("texto");
        assert!(message.contains("soulmate"), "{message}");
        assert!(!message.contains("mood.affinity.level"), "chave de tradução vazou: {message}");
    }
}
