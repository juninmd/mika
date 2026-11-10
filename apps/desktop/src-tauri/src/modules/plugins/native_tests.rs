//! **O que estes testes NÃO cobrem, e por quê.** `invoke` recebe `AppHandle`, que não existe fora
//! de um app Tauri de verdade (a feature `test` do crate não está ligada aqui). Então o
//! comportamento é exercido em `rpc::dispatch`, que é onde ele mora — este módulo é só o fio — e o
//! fio em si é provado lendo o fonte, igual `tools/native_tests.rs` faz pelo mesmo motivo.

use super::*;
use crate::protocol::WindowType;
use serde_json::json;

/// Os 25 métodos RPC que os 4 arquivos TS (removidos — backend Node deletado na migração para
/// Tauri) registravam de verdade, congelados no momento da deleção. `character`, `desktop`, `mcp`,
/// `privacy`, `scheduler` e `user` não registravam método nenhum, apesar de aparecerem na lista de
/// "9 plugins tardios".
const FROZEN_TS_METHODS: &[&str] = &[
    "session-list",
    "session-current",
    "steam:top-games",
    "steam:game-art",
    "mini-game:payload",
    "mini-game:requested",
    "mini-game:result",
    "mood_get",
    "mood_update",
    "affinity_get",
    "affinity_add_interaction",
    "person_catalog_image",
    "person_catalog_list",
    "person_catalog_search",
    "person_catalog_get",
    "person_catalog_upsert",
    "person_catalog_merge",
    "person_catalog_block",
    "person_catalog_unblock",
    "person_catalog_blocked_list",
    "person_catalog_delete",
    "brain_get_social_facts",
    "brain_add_social_fact",
    "brain_delete_social_fact",
    "brain_update_social_fact",
];

const RUST_SOURCES: &[&str] = &[
    include_str!("../gamer/rpc.rs"),
    include_str!("../features/mood/rpc.rs"),
    include_str!("../features/person_catalog/rpc.rs"),
    include_str!("../memory/social/rpc.rs"),
];

fn registered_in_rust() -> String {
    RUST_SOURCES.iter().map(|source| source.split("#[cfg(test)]").next().unwrap_or_default()).collect()
}

#[test]
fn claims_plugin_rpc_and_nothing_else() {
    // `plugin-test` é do módulo `settings` (o ramo de teste de conexão mora lá); reivindicá-lo aqui
    // roubaria o canal de quem tem a implementação, e `bridge.rs` roteia por canal inteiro.
    assert_eq!(PluginRpcModule.channels(), &["mika:plugin-rpc"]);
}

/// O par do teste acima: o canal declarado tem de ter braço em `invoke`. Declarar e não tratar
/// devolveria "canal desconhecido" para a rota inteira, com o sidecar já cortado.
#[test]
fn every_claimed_channel_has_a_branch_in_invoke() {
    let source = include_str!("native.rs");
    let production = source.split("#[cfg(test)]").next().unwrap_or_default();
    for channel in PluginRpcModule.channels() {
        assert!(
            production.contains(&format!("const CH_PLUGIN_RPC: &str = \"{channel}\"")),
            "`{channel}` é reivindicado sem constante correspondente em native.rs"
        );
    }
    assert!(production.contains("CH_PLUGIN_RPC => Ok("), "o braço de mika:plugin-rpc sumiu de invoke");
}

/// **A direção que importa.** O canal é reivindicado por INTEIRO (`state.native.owns(&channel)`),
/// então um método que o TS declara e o Rust não registra não cai para o sidecar — ele some da
/// tela, com `handler-missing` que o renderer costuma tratar como "vazio". Método a mais em Rust
/// não quebra ninguém; método a MENOS quebra uma feature em silêncio.
///
/// Lê o fonte Rust em vez de contar em `registry()`: aquele registro é `OnceLock` de processo e
/// `cargo test` roda em paralelo, onde contagem vira flake por vizinho.
#[test]
fn every_frozen_ts_method_is_registered_in_rust() {
    let registered = registered_in_rust();
    let missing: Vec<&&str> = FROZEN_TS_METHODS
        .iter()
        .filter(|method| !registered.contains(&format!("\"{method}\"")))
        .collect();

    assert!(
        missing.is_empty(),
        "métodos que o TS declara e o Rust não registra: {missing:?} — com o canal já nativo eles não \
         descem para o sidecar, somem da tela"
    );
}

#[tokio::test]
async fn an_unknown_method_answers_the_envelope_instead_of_rejecting() {
    let response = crate::modules::plugins::rpc::dispatch(WindowType::Main, None, &json!({ "pluginId": "mika.nao-existe", "method": "seja_o_que_for" })).await;
    assert_eq!(response["success"], json!(false));
    assert_eq!(response["error"]["code"], json!("handler-missing"));
}

#[tokio::test]
async fn a_plugin_view_cannot_reach_another_plugins_methods() {
    let payload = json!({ "pluginId": "mika.person-catalog", "method": "person_catalog_delete", "args": { "id": "x" } });
    let response = crate::modules::plugins::rpc::dispatch(WindowType::PluginView, Some("mika.gamer-companion"), &payload).await;
    assert_eq!(response["error"]["code"], json!("unauthorized"));
}

#[tokio::test]
async fn a_malformed_request_is_invalid_not_missing() {
    // `pluginId` fora do formato `mika.*` — o mesmo regex de `pluginRpcRequestSchema`.
    let response = crate::modules::plugins::rpc::dispatch(WindowType::Main, None, &json!({ "pluginId": "../etc/passwd", "method": "list" })).await;
    assert_eq!(response["error"]["code"], json!("invalid-request"));
}
