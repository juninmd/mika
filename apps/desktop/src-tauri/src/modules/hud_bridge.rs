//! Ponte HUD: o que o barramento publica e a TELA precisa ver.
//!
//! O lado TS tinha uma ponte genérica (`infrastructure/renderer-event-bridge.ts` +
//! `DESKTOP_BRIDGED_EVENT_CHANNELS`) que levava todo evento nomeado ao renderer. No Rust cada
//! ponte nasceu sob demanda e específica — `chat::assistant_bridge` (texto do recap/debate) e
//! `features::character::expression_bridge` (pose do avatar) —, e o resto do catálogo ficou sem
//! ninguém: `Notification`, `ListeningStarted` e `ListeningStopped` eram publicadas e não chegavam
//! a lugar nenhum.
//!
//! O efeito era invisível do jeito mais caro: o indicador de microfone da HUD nunca acendia, e
//! todo aviso ao usuário (queda de provedor, subida de nível de afinidade, lembrete) era escrito
//! no barramento e descartado. Os dois lados com teste verde — o publicador prova que publica, e o
//! consumidor que não existe não tem o que provar. `check:rust-event-consumers` passou a cruzar as
//! duas listas por isso.
//!
//! **Uma ponte com tabela, não três pontes.** Toda chave aqui tem o mesmo destino (as janelas do
//! dono) e a mesma tradução (`EventKey` -> canal do contrato). Chave com política própria
//! continua tendo ponte própria: `chat:assistant-message` decide alvo por janela e `Expression`
//! carrega regra de avatar.
//!
//! **Os literais vêm de `DESKTOP_BRIDGED_EVENT_CHANNELS`** (`packages/sdk/src/contracts/
//! desktop-ipc-channels.ts`), o mesmo arquivo que o renderer lê. O teste no fim deste arquivo LÊ
//! esse fonte: um literal divergente aqui deixaria a HUD muda sem erro nenhum, que é a classe de
//! defeito que `bridge.rs::shell_channels_match_typescript_contract` já pegou uma vez.

use tauri::{AppHandle, Emitter, Manager};

use crate::events::{EventBus, EventKey};
use crate::protocol::renderer_event_targets;

/// `EventKey` -> canal que o renderer escuta. Espelha `DESKTOP_BRIDGED_EVENT_CHANNELS`.
const BRIDGED: &[(EventKey, &str)] = &[
    (EventKey::Notification, "hud:notification"),
    (EventKey::ListeningStarted, "hud:listening-started"),
    (EventKey::ListeningStopped, "hud:listening-stopped"),
    // Port de `ExpressionSyncService` (`modules/voice/expression_sync.rs`) — canal já esperado
    // pela UI em `DESKTOP_BRIDGED_EVENT_CHANNELS.Activity`.
    (EventKey::ActivityChanged, "hud:activity"),
    // `EventBusSink::on_chunk_ready` (`voice/tts/bus_bridge.rs`) publica o áudio sintetizado de
    // verdade desde a rodada da tool `speak`, mas nada o encaminhava ao renderer — a Mika falava
    // no processo Rust e o áudio nunca saía dele. `mika:event:tts:audio` é o canal que
    // `onTTSAudio` (`renderer/bridge/mika-runtime-chat.ts`) já espera, prefixo `mika:event:` (não
    // `hud:`) porque este canal é mais antigo que a convenção HUD — mesma família de
    // `DESKTOP_EVENT_CHANNELS`, não `DESKTOP_BRIDGED_EVENT_CHANNELS`, mas o teste abaixo só checa
    // que o literal existe em algum lugar do contrato, o que ele faz.
    (EventKey::TtsAudioReady, "mika:event:tts:audio"),
    // ElevenLabs caindo para a voz local (`voice/tts/dispatch.rs`) — canal já reservado no
    // contrato (`DESKTOP_BRIDGED_EVENT_CHANNELS.TtsFallback`), sem publicador Rust até esta rodada.
    (EventKey::TtsFallback, "hud:tts-fallback"),
    // Poller de energia (`system_signals/power.rs`) — a UI já tinha os dois lados do badge
    // (`onPowerModeChanged`), só o lado Rust->renderer estava faltando.
    (EventKey::PowerBattery, "hud:power-battery"),
    (EventKey::PowerAc, "hud:power-ac"),
    // Catálogo de pessoas — `person_catalog::tool.rs` grava de verdade desde sempre, mas nunca
    // avisava a tela aberta. `PersonMentionAdded` reusa o MESMO canal de `PersonUpdated` de
    // propósito: `mika-event-channels.ts` já trata os dois como o mesmo sinal de "recarregue a
    // lista" (`'person-catalog:mention-added': MIKA_EVENT_CHANNELS.personCatalogUpdated`).
    (EventKey::PersonUpdated, "hud:person-catalog-updated"),
    (EventKey::PersonMentionAdded, "hud:person-catalog-updated"),
    (EventKey::PersonDeleted, "hud:person-catalog-deleted"),
    // Vision live monitor and SensesMonitor (`onScreenObservation`) never received a frame: the
    // watcher publishes on the bus, and nothing forwarded it.
    (EventKey::ScreenObservation, "mika:event:vision:screen-observation"),
    // Published on the bus but never forwarded: the mood HUD, the barge-in audio stop, the
    // proactive-thought bubble and the chat error toast stayed silent.
    (EventKey::MoodChanged, "hud:mood-changed"),
    (EventKey::ForceStopTts, "hud:force-stop-tts"),
    (EventKey::BrainProactiveThought, "hud:brain-proactive"),
    (EventKey::ChatError, "mika:event:chat:error"),
    // defects.md "barge-in", item 1: o renderer precisa do `transcript`/`timestamp`/`window_label`
    // do barge-in para descartar seu próprio envio da MESMA fala (`interruption-dedupe.ts`) — sem
    // isto o único consumidor era o Rust (`auto_speak`/`awareness`), e a UI nunca via o evento.
    (EventKey::Interruption, "hud:interruption"),
    // "Pausar mic e tela" da bandeja (`privacy/senses.rs`): o HUD/SensesMonitor mostram os sentidos desligados.
    (EventKey::SensesPaused, "hud:senses-paused"),
];

fn channel_for(key: EventKey) -> Option<&'static str> {
    BRIDGED.iter().find(|(bridged, _)| *bridged == key).map(|(_, channel)| *channel)
}

/// Assina o barramento e reenvia cada chave da tabela às janelas do dono. Chamada em
/// `modules::register_config_bound_tools` — o ponto com `AppHandle`, mesmo motivo de
/// `chat::spawn_assistant_message_bridge` estar lá.
pub fn spawn_hud_bridge(bus: &'static EventBus, app: AppHandle) {
    // Assina ANTES do spawn: `broadcast` não entrega retroativamente, e `spawn` só agenda a task.
    let mut sub = bus.subscribe();
    // `tauri::async_runtime::spawn`, nunca `tokio::spawn`: esta função roda em `lib.rs::setup`,
    // fora do contexto do reator (ver o gate em `modules/mod.rs`).
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
            let Some(channel) = channel_for(envelope.key) else {
                continue;
            };
            for label in renderer_event_targets(app.webview_windows().keys().cloned().collect()) {
                let _ = app.emit_to(label.as_str(), channel, envelope.payload.clone());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keys the TS forwarder bridged that reach the renderer through their own path instead: the
    /// turn IPC of `chat::send` (thinking/tool events) or a dedicated bridge.
    const FORWARDED_ELSEWHERE: &[EventKey] = &[
        EventKey::AssistantMessage,
        EventKey::Expression,
        EventKey::ThinkingStart,
        EventKey::ThinkingStop,
        EventKey::StreamChunk,
        EventKey::ToolCall,
        EventKey::ToolResult,
    ];

    /// Canais bridged que o TS espera sem NENHUM `EventKey` Rust correspondente — não é "esquecido
    /// de encaminhar", é "não existe produtor nenhum do lado Rust para isto". Cada entrada aqui
    /// precisa de um comentário com o motivo; a lista vazia é o estado saudável, não um atalho.
    const NO_RUST_KEY_ALLOWLIST: &[&str] = &[];

    /// Varre o bloco `MIKA_BRIDGED_EVENT_CHANNELS` e devolve, para cada canal, por que ele falha:
    /// sem `EventKey` Rust nenhum (bug de catálogo — produtor não existe), ou com `EventKey` mas
    /// sem ponte (bug de fiação — produtor existe, ninguém encaminha). Extraída da asserção para
    /// caber num teste negativo que não precisa ler o arquivo rastreado (ver o teste abaixo).
    fn missing_forwards(ts_source: &str) -> Vec<String> {
        let block = ts_source.split("MIKA_BRIDGED_EVENT_CHANNELS").nth(1).expect("bridged map");
        block
            .lines()
            .take_while(|line| !line.starts_with('}'))
            .filter_map(|line| line.trim().split_once(": MIKA_EVENT_CHANNELS.").map(|(name, _)| name.trim_matches('\'')))
            .filter(|name| !NO_RUST_KEY_ALLOWLIST.contains(name))
            .filter_map(|name| match EventKey::ALL.iter().find(|key| key.as_str() == name) {
                // Nome sem `EventKey` correspondente — a versão antiga deste teste descartava isto
                // em silêncio (`filter_map` sem ramo `None`), então um canal sem produtor Rust
                // nenhum passava verde. É exatamente essa lacuna que `channel:message` explorou.
                None => Some(format!("{name} (sem EventKey Rust nenhum)")),
                Some(key) if channel_for(*key).is_none() && !FORWARDED_ELSEWHERE.contains(key) => Some(name.to_string()),
                Some(_) => None,
            })
            .collect()
    }

    /// Every bus event the renderer's bridged map expects must reach it; a missing row here is a
    /// HUD that never updates, with no error anywhere.
    #[test]
    fn every_bridged_typescript_event_with_a_rust_key_is_forwarded() {
        let ts = include_str!("../../../src/shared/mika-event-channels.ts");
        let missing = missing_forwards(ts);
        assert!(missing.is_empty(), "eventos que o renderer espera e ninguém encaminha: {missing:?}");
    }

    /// Sonda negativa: prova que um canal SEM `EventKey` nenhum reprova o teste acima, sem editar
    /// `mika-event-channels.ts` (arquivo rastreado) para simular o bug — a entrada fake vive só
    /// nesta string local.
    #[test]
    fn missing_forwards_flags_a_channel_with_no_rust_key_at_all() {
        let fake_ts = "export const MIKA_BRIDGED_EVENT_CHANNELS = {\n  'fake:channel-nobody-forwards': MIKA_EVENT_CHANNELS.fakeChannel,\n} as const\n";
        assert_eq!(missing_forwards(fake_ts), vec!["fake:channel-nobody-forwards (sem EventKey Rust nenhum)".to_string()]);
    }

    #[test]
    fn only_the_table_is_forwarded() {
        assert_eq!(channel_for(EventKey::Notification), Some("hud:notification"));
        assert_eq!(channel_for(EventKey::ListeningStarted), Some("hud:listening-started"));
        assert_eq!(channel_for(EventKey::ActivityChanged), Some("hud:activity"));
        assert_eq!(channel_for(EventKey::TtsAudioReady), Some("mika:event:tts:audio"));
        assert_eq!(channel_for(EventKey::TtsFallback), Some("hud:tts-fallback"));
        assert_eq!(channel_for(EventKey::PowerBattery), Some("hud:power-battery"));
        assert_eq!(channel_for(EventKey::PowerAc), Some("hud:power-ac"));
        assert_eq!(channel_for(EventKey::PersonUpdated), Some("hud:person-catalog-updated"));
        assert_eq!(channel_for(EventKey::PersonMentionAdded), Some("hud:person-catalog-updated"));
        assert_eq!(channel_for(EventKey::PersonDeleted), Some("hud:person-catalog-deleted"));
        assert_eq!(channel_for(EventKey::ScreenObservation), Some("mika:event:vision:screen-observation"));
        assert_eq!(channel_for(EventKey::Interruption), Some("hud:interruption"));
        // Quem tem ponte própria não pode ser reenviado aqui também: a tela receberia a mesma
        // mensagem duas vezes, por dois caminhos com políticas de alvo diferentes.
        assert_eq!(channel_for(EventKey::AssistantMessage), None);
        assert_eq!(channel_for(EventKey::Expression), None);
        assert_eq!(channel_for(EventKey::VadAudio), None);
    }

    /// Literal errado = HUD muda, sem erro e sem log. O contrato é o mesmo arquivo que o renderer
    /// importa, então ele é lido aqui em vez de confiado de memória.
    #[test]
    fn every_channel_literal_exists_in_the_typescript_contract() {
        let contract = include_str!("../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        for (_, channel) in BRIDGED {
            assert!(
                contract.contains(&format!("'{channel}'")),
                "canal '{channel}' não aparece em desktop-ipc-channels.ts — o Rust emitiria num canal que ninguém escuta"
            );
        }
    }
}
