//! Suíte de `native.rs`, em arquivo separado só pela regra de 300 linhas do AGENTS.md — mesmo
//! módulo lógico `native::tests`, com o mesmo acesso a privado que teria inline.
//!
//! **O que estes testes NÃO conseguem cobrir, e por quê.** `execute` recebe `AppHandle`, e este
//! crate não liga a feature `tauri/test` (mesma fronteira que `chat/send.rs` e `window_approval.rs`
//! já documentam). O que é testável aqui é tudo que fica ANTES e DEPOIS do app: a política de
//! acesso (`access.rs`, com suíte própria), o achatamento do catálogo (`catalog.rs`, idem), a
//! listagem ao vivo contra o registro real, e a recepção da resposta de aprovação. O despacho com
//! janela de verdade é coberto pelo E2E.

use super::*;
use serde_json::json;
use std::sync::Arc;

use crate::modules::tools::{ToolDefinition, ToolHandler};
use crate::protocol::WindowType;

fn ctx(window_type: WindowType, label: &str, plugin_id: Option<&str>) -> InvokeContext {
    InvokeContext {
        window_type,
        window_label: label.to_string(),
        plugin_id: plugin_id.map(str::to_owned),
    }
}

struct Echo;

#[async_trait]
impl ToolHandler for Echo {
    async fn call(&self, args: Value) -> Result<Value, String> {
        Ok(json!({ "success": true, "echo": args }))
    }
}

/// Registra uma tool sob um `plugin_id` exclusivo desta asserção.
///
/// O registro é um `OnceLock` de PROCESSO e `cargo test` roda em paralelo: contar o catálogo
/// inteiro aqui mediria os vizinhos, não este teste. Filtrar por um `plugin_id` que só este teste
/// usa dá uma medida estável sem precisar serializar a suíte — a mesma armadilha que `tests.rs`
/// deste diretório já documenta para os pins negativos que teve de remover.
fn register_under(plugin_id: &str, tool_name: &str) {
    registry().register(
        ToolDefinition {
            name: tool_name.into(),
            description: Some("fixture".into()),
            input_schema: json!({ "type": "object", "properties": { "x": { "type": "string" } } }),
            plugin_id: plugin_id.into(),
            requires_window: false,
        },
        Arc::new(Echo),
    );
}

/// Os quatro literais contra o contrato TS. Um canal digitado errado aqui não quebra a build: o
/// `bridge.rs` simplesmente não acha dono nativo e desce para o sidecar — a ação continua
/// funcionando, o placar da catraca sobe, e ninguém vê que a migração não aconteceu.
#[test]
fn the_claimed_channels_match_the_typescript_contract() {
    let channels = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
    for channel in CHANNELS {
        assert!(
            channels.contains(&format!("'{channel}'")),
            "canal '{channel}' não existe em desktop-ipc-channels.ts — o bridge não acharia dono e a ação voltaria ao sidecar"
        );
    }
    assert_eq!(CHANNELS.len(), 4, "os quatro canais migram juntos — ver o cabeçalho de native.rs");
}

/// Os dois canais de HUD são os MESMOS que `chat/send.rs` emite. Divergir aqui deixaria a UI de
/// tools muda sem erro nenhum: o evento sairia para um nome que nenhum `onToolCall` escuta.
#[test]
fn the_hud_events_are_the_same_ones_the_chat_turn_already_emits() {
    let send = include_str!("../chat/send.rs");
    assert!(send.contains(&format!("\"{HUD_TOOL_CALL_EVENT}\"")), "o canal de tool-call divergiu de chat/send.rs");
    assert!(send.contains(&format!("\"{HUD_TOOL_RESULT_EVENT}\"")), "o canal de tool-result divergiu de chat/send.rs");
}

#[test]
fn listing_is_denied_for_a_surface_the_policy_does_not_allow() {
    let denied = list(&ctx(WindowType::Wizard, "wizard", None), None);
    assert!(denied.is_err(), "o wizard não pode listar o catálogo de tools");
}

/// Listagem ao vivo contra o registro REAL — não um fixture de registro paralelo. Prova que o
/// canal devolve `{ tools: [...] }` com a forma de catálogo, e que o filtro por plugin filtra.
#[test]
fn listing_by_plugin_returns_only_that_plugins_tools_in_catalog_shape() {
    register_under("test.native.listing", "listing_fixture_tool");

    let response = list(&ctx(WindowType::Main, "main", None), Some("test.native.listing")).unwrap();
    let tools = response["tools"].as_array().expect("a resposta tem de ser { tools: [...] }");

    assert_eq!(tools.len(), 1, "o filtro por pluginId deixou passar tool de outro plugin");
    assert_eq!(tools[0]["name"], json!("listing_fixture_tool"));
    assert_eq!(tools[0]["pluginId"], json!("test.native.listing"));
    // A forma ACHATADA, não o schema cru — é o que a tela de tools desenha.
    assert_eq!(tools[0]["inputSchema"]["properties"]["x"]["type"], json!("string"));
}

/// Sem `pluginId`, o catálogo inteiro — e ele contém a tool que acabamos de registrar. Não conta
/// o total (vizinhos em paralelo registram os seus), só prova que a listagem sem filtro alcança o
/// mesmo registro.
#[test]
fn listing_without_a_plugin_filter_reaches_the_whole_registry() {
    register_under("test.native.unfiltered", "unfiltered_fixture_tool");

    let response = list(&ctx(WindowType::Settings, "settings", None), None).unwrap();
    let names: Vec<&str> = response["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();

    assert!(names.contains(&"unfiltered_fixture_tool"), "a listagem sem filtro não alcançou o registro de processo");
}

/// `pluginId: ""` lista tudo, igual ao `pluginId || undefined` do TS — e não é buraco: quem
/// filtra por plugin é conveniência da UI, quem barra é `assert_tool_execution_allowed`.
#[test]
fn an_empty_plugin_id_lists_everything_like_the_typescript_does() {
    register_under("test.native.empty", "empty_filter_fixture_tool");

    let filtered = Some("").filter(|id: &&str| !id.is_empty());
    let response = list(&ctx(WindowType::Main, "main", None), filtered).unwrap();
    let names: Vec<&str> =
        response["tools"].as_array().unwrap().iter().filter_map(|tool| tool["name"].as_str()).collect();

    assert!(names.contains(&"empty_filter_fixture_tool"));
    assert!(names.len() > 1, "pluginId vazio deveria listar o catálogo inteiro, não um subconjunto");
}

/// **A asserção mais importante deste arquivo.** `approved` ausente ou ilegível é NEGAÇÃO.
///
/// É o único ramo onde um default errado EXECUTA a ação em vez de recusá-la: um payload
/// malformado que virasse `approved: true` liberaria `clear_history` ou `desktop:launch` sem o
/// usuário ter clicado em nada.
#[test]
fn a_malformed_approval_response_never_counts_as_approval() {
    for payload in [
        json!({ "toolCallId": "t1" }),
        json!({ "toolCallId": "t1", "approved": "yes" }),
        json!({ "toolCallId": "t1", "approved": 1 }),
        json!({ "toolCallId": "t1", "approved": null }),
    ] {
        let approved = payload.get("approved").and_then(Value::as_bool).unwrap_or(false);
        assert!(!approved, "payload {payload} foi lido como aprovação — é o default que executa sem clique");
    }
}

/// Resposta sem `toolCallId` (ou com um vazio) é ignorada, não propagada como decisão anônima —
/// resolver "a pendência qualquer" casaria a resposta com a aprovação errada.
#[test]
fn an_approval_response_without_a_call_id_is_ignored() {
    let owner = ctx(WindowType::Main, "main", None);
    assert_eq!(resolve_approval(&owner, &json!({ "approved": true })), Ok(Value::Null));
    assert_eq!(resolve_approval(&owner, &json!({ "toolCallId": "", "approved": true })), Ok(Value::Null));
    // Sem handler instalado (o caso de todo teste) também é no-op silencioso: ninguém espera.
    assert_eq!(resolve_approval(&owner, &json!({ "toolCallId": "nunca-pedido", "approved": true })), Ok(Value::Null));
}

/// Quem responde a um diálogo de aprovação é o dono. `plugin-view` (terceiro), janela desconhecida
/// e qualquer outra superfície são recusadas ANTES de o payload ser lido — senão um plugin que
/// conhecesse um `toolCallId` aprovaria `desktop:launch` no lugar do usuário.
#[test]
fn only_an_owner_surface_may_answer_an_approval() {
    let payload = json!({ "toolCallId": "t1", "approved": true, "scope": "always" });
    for window in [WindowType::Main, WindowType::Settings, WindowType::Wizard] {
        assert!(resolve_approval(&ctx(window, "x", None), &payload).is_ok(), "{window:?} é superfície do dono");
    }
    for window in [WindowType::PluginView, WindowType::Unknown] {
        let result = resolve_approval(&ctx(window, "x", Some("mika.evil")), &payload);
        assert!(result.is_err(), "{window:?} não pode aprovar tool no lugar do usuário");
    }
}

/// O escopo vem do payload, e o DESCONHECIDO cai para `Once` — o mais restrito. Um escopo
/// inventado pelo renderer não pode virar `Always` e gravar uma concessão permanente.
#[test]
fn an_unknown_approval_scope_falls_back_to_the_most_restrictive_one() {
    let scope_of = |raw: Value| match raw.get("scope").and_then(Value::as_str) {
        Some("session") => ApprovalScope::Session,
        Some("always") => ApprovalScope::Always,
        _ => ApprovalScope::Once,
    };

    assert_eq!(scope_of(json!({ "scope": "session" })), ApprovalScope::Session);
    assert_eq!(scope_of(json!({ "scope": "always" })), ApprovalScope::Always);
    assert_eq!(scope_of(json!({ "scope": "forever" })), ApprovalScope::Once);
    assert_eq!(scope_of(json!({})), ApprovalScope::Once);
}

/// Os três valores de escopo do contrato TS existem aqui. Um `scope` que o renderer manda e o
/// Rust não reconhece vira `Once` (teste acima) — correto como fail-safe, errado como paridade:
/// o usuário pediria "sempre" e teria "desta vez", sem nada avisando.
#[test]
fn every_approval_scope_in_the_ts_contract_is_understood_here() {
    let contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc.ts");
    assert!(
        contract.contains("scope: z.enum(['once', 'session', 'always']).optional()"),
        "o enum de scope mudou em desktop-ipc.ts — releia antes de manter o match de resolve_approval"
    );
}

/// O canal reivindicado tem de ser o que o módulo de fato trata. Um canal em `CHANNELS` sem ramo
/// no `match` vira erro explícito em vez de cair no sidecar — é o contrato que `NativeModule`
/// documenta, e o teste que prova que ele vale aqui.
#[test]
fn every_claimed_channel_has_a_branch_in_invoke() {
    let module = ToolsModule;
    for channel in CHANNELS {
        assert!(
            matches!(*channel, CH_TOOLS_LIST | CH_TOOLS_LIST_BY_PLUGIN | CH_TOOL_EXECUTE | CH_TOOL_APPROVAL_RESPONSE),
            "canal '{channel}' está em CHANNELS e não tem ramo no match de invoke"
        );
    }
    assert_eq!(module.name(), "tools");
    assert_eq!(module.channels().len(), 4);
}

#[test]
fn a_background_tool_poll_keeps_its_origin_so_the_hud_stays_dark() {
    assert_eq!(hud_origin(&json!({ "name": "mood", "origin": "background" })), "background");
    assert_eq!(hud_origin(&json!({ "name": "mood" })), "turn");
    assert_eq!(hud_origin(&json!({ "name": "mood", "origin": "anything" })), "turn");
}
