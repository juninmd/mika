//! Ponte EventBus -> IPC para `EventKey::Expression` — espelha `modules/chat/assistant_bridge.rs`
//! para um único canal. Sem isto, a tool `avatar` (`tool.rs::run_expression`) publica no barramento
//! de processo e nenhuma janela jamais recebe: o avatar renderizado nunca troca de pose.
//!
//! **Canal**: `MIKA_BRIDGED_EVENT_CHANNELS['avatar:expression']`
//! (`apps/desktop/src/shared/mika-event-channels.ts`) resolve para
//! `DESKTOP_EVENT_CHANNELS.AvatarExpression` (`packages/sdk/src/contracts/desktop-ipc-channels.ts`)
//! = `'mika:event:avatar:expression'`. `mika-runtime-*`/`useAvatarExpression` do lado renderer é
//! quem escuta esse canal.
//!
//! **`emit_to` nos alvos do TS, nunca `app.emit`.** `avatar:expression` está FORA de
//! `ALL_WINDOW_EVENTS` no TS (mesmo grupo de `chat:assistant-message`) — `sendRendererEvent` manda
//! só para a janela `main` quando ela existe, e no fallback filtra `plugin-view`. [`select_targets`]
//! é a MESMA regra que `assistant_bridge::select_targets` já prova — reimplementada aqui (não
//! importada) porque `chat::assistant_bridge` é privado ao próprio módulo `chat`, e a regra pura em
//! si é só duas linhas; o que importa é o CONTRATO batendo com o TS, provado abaixo pelo mesmo
//! conjunto de casos.
//!
//! `plugin-view` é a superfície que este repositório trata como não confiável (`is_owner_surface`
//! barra 10 rotas de IPC nela) — alcançá-la com `app.emit` alargaria superfície por acidente de
//! API, sem rota nova e sem nada falhando, exatamente como o cabeçalho de `assistant_bridge.rs`
//! documenta.
//!
//! **Sem teste de fiação real.** `app.emit_to` exige um `AppHandle` de verdade; este crate não
//! habilita a feature `test-support` do `tauri`. O que fica testável sem isso — o filtro de chave e
//! a política de alvo — está coberto abaixo, mesma lacuna nomeada de `assistant_bridge.rs`.

use tauri::{AppHandle, Emitter, Manager};

use crate::events::{EventBus, EventKey};
use crate::protocol::renderer_event_targets;

/// Espelha `DESKTOP_EVENT_CHANNELS.AvatarExpression`.
const EXPRESSION_IPC_EVENT: &str = "mika:event:avatar:expression";

fn is_expression(key: EventKey) -> bool {
    key == EventKey::Expression
}

/// Assina `bus` e reenvia cada `EventKey::Expression` para os alvos corretos. Chamado só em
/// produção (`modules/mod.rs::register_config_bound_tools`, o único ponto com `AppHandle`) — mesmo
/// padrão de `chat::spawn_assistant_message_bridge`/`voice::tts::bus_bridge::spawn_force_stop_listener`.
pub fn spawn_expression_bridge(bus: &'static EventBus, app: AppHandle) {
    // Assina ANTES do `tokio::spawn`, não dentro do bloco async: `broadcast` não entrega
    // retroativamente, e `tokio::spawn` só AGENDA a task — mesmo aviso de `assistant_bridge.rs`.
    let mut sub = bus.subscribe();
    // `tauri::async_runtime::spawn`, NUNCA `tokio::spawn` — ver o gate em `modules/mod.rs`: esta
    // função roda em `lib.rs::setup`, fora do contexto do reator, e `tokio::spawn` ali panica.
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
            if is_expression(envelope.key) {
                for label in expression_targets(&app) {
                    let _ = app.emit_to(label.as_str(), EXPRESSION_IPC_EVENT, envelope.payload.clone());
                }
            }
        }
    });
}

/// A regra em si vive em [`crate::protocol::renderer_event_targets`], compartilhada com
/// `chat::assistant_bridge` — ver o cabeçalho de lá para por que uma regra de confiança não pode
/// ter duas cópias. Os casos que a provam moram junto dela.
fn expression_targets(app: &AppHandle) -> Vec<String> {
    renderer_event_targets(app.webview_windows().keys().cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_expression_is_forwarded() {
        assert!(is_expression(EventKey::Expression));
        assert!(!is_expression(EventKey::Notification));
        assert!(!is_expression(EventKey::MoodChanged));
    }

    /// Um literal errado aqui deixaria o avatar mudo para sempre, sem erro nenhum — mesma checagem
    /// de `assistant_bridge.rs::channel_literal_exists_in_the_ts_contract`.
    #[test]
    fn channel_literal_exists_in_the_ts_contract() {
        let channels_contract = include_str!("../../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        assert!(
            channels_contract.contains(&format!("'{EXPRESSION_IPC_EVENT}'")),
            "canal {EXPRESSION_IPC_EVENT} não existe em desktop-ipc-channels.ts"
        );
    }

    /// Prova que o canal Rust resolve para a MESMA chave que `MIKA_BRIDGED_EVENT_CHANNELS`
    /// (`apps/desktop/src/shared/mika-event-channels.ts`) mapeia para `avatar:expression` — sem
    /// isto, um literal certo aqui e errado lá (ou vice-versa) passaria os dois testes isolados.
    #[test]
    fn expression_key_maps_to_this_channel_in_the_bridge_contract() {
        let bridge_contract = include_str!("../../../../../../../apps/desktop/src/shared/mika-event-channels.ts");
        assert!(
            bridge_contract.contains("'avatar:expression': MIKA_EVENT_CHANNELS.avatarExpression"),
            "mika-event-channels.ts não mapeia mais 'avatar:expression' para avatarExpression"
        );
    }

    /// Mesmo gate de fonte de `assistant_bridge.rs`: a regra de alvo é compartilhada, e `app.emit`
    /// alcançaria `plugin-view` com sucesso e em silêncio. Ver o cabeçalho de
    /// `protocol::renderer_event_targets`.
    #[test]
    fn the_bridge_routes_through_the_shared_surface_rule_and_never_broadcasts() {
        let source = include_str!("expression_bridge.rs");
        let production: String = source
            .split("mod tests {")
            .next()
            .unwrap_or_default()
            .lines()
            .map(|line| line.split("//").next().unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(production.contains("renderer_event_targets("), "a ponte deixou de usar a regra compartilhada de alvo");
        assert!(!production.contains("app.emit("), "`app.emit` alcança plugin-view — use `emit_to` nos alvos de renderer_event_targets");
    }
}
