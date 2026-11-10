//! Guarda do MUST em `specs/01-architecture.md` §"Isolamento de processo": "toda rota IPC valida
//! o payload na fronteira antes de agir". `bridge.rs` não tem um router/Zod único (ver o
//! `[Parcial no porte Tauri]` daquele spec) — a validação é por handler, espalhada por 13 módulos.
//! Sem este arquivo, um canal novo sem validação nenhuma não reprova NADA: compila, roda, e só
//! falha (ou pior, aceita entrada hostil) em produção.
//!
//! **O que este teste prova, e o que não prova.** Não há como o compilador confirmar sozinho que
//! um handler "valida o payload" — isso é julgamento humano sobre cada `match`. O que dá para
//! automatizar é a CONTABILIDADE: toda rota que o `NativeRegistry` (mais os `SHELL_CHANNELS` de
//! `bridge.rs`, que nunca passam pelo registro) reivindica em produção tem de ter uma entrada
//! NOMEADA aqui, com o veredito (`Validated`/`Allowlisted`) e a razão. Um canal novo sem entrada
//! reprova `every_active_channel_has_an_audit_entry` — obrigando quem o acrescentar a vir aqui
//! decidir e justificar, em vez de o gap ficar invisível.
//!
//! Direção única de propósito: todo canal ATIVO precisa de entrada, mas uma entrada sem canal
//! ativo (`mika:e2e-hook`, que só existe com `MIKA_E2E=1`) não é erro — é o mesmo canal, só que a
//! maioria das rodadas de `cargo test` roda sem a variável.

#[derive(Debug, Clone, Copy)]
enum RouteAudit {
    /// O handler valida tipo/forma/limite do payload antes de agir — a razão aponta ONDE.
    Validated(&'static str),
    /// Payload vazio, ignorado, ou cujo pior caso é inofensivo (leitura sem efeito colateral,
    /// ação sem parâmetro) — a razão explica por que não há o que validar.
    Allowlisted(&'static str),
}

/// Uma entrada por canal REIVINDICADO em produção — `bridge.rs::SHELL_CHANNELS` mais todo
/// `NativeModule::channels()` de `crate::modules::all()`. Revisado handler a handler nesta rodada;
/// ver o diff que introduziu este arquivo para o rastro de cada veredito.
const ROUTE_AUDIT: &[(&str, RouteAudit)] = &[
    // -- bridge.rs::SHELL_CHANNELS — resolvidos direto em `bridge.rs`, nunca no NativeRegistry ----
    ("open-settings", RouteAudit::Allowlisted("payload ignorado; abre a janela de settings sem parâmetro")),
    ("window-close", RouteAudit::Allowlisted("payload ignorado; fecha a janela chamadora")),
    ("window-toggle-always-on-top", RouteAudit::Allowlisted("payload ignorado; alterna estado interno")),
    ("window-toggle-fullscreen", RouteAudit::Allowlisted("payload ignorado; alterna `app.fullscreen` a partir do estado real da janela")),
    ("window-get-fullscreen", RouteAudit::Allowlisted("payload ignorado; leitura do estado da janela")),
    ("window-ignore-mouse", RouteAudit::Validated("bridge.rs: payload.as_bool() obrigatório")),
    ("window-move", RouteAudit::Validated("bridge.rs: deltaX/deltaY exigidos como f64")),
    ("window-start-drag", RouteAudit::Allowlisted("payload ignorado; inicia o drag nativo da janela")),
    ("plugin-view:open", RouteAudit::Validated("shell_actions::handle: pluginId/viewId contra regex [a-z0-9.-]")),
    ("open-external", RouteAudit::Validated("shell_actions::external_url_allowed: https sem credencial/porta e host na allowlist EXTERNAL_HOSTS")),
    ("reveal-path", RouteAudit::Validated("shell_actions::reveal_target: kind por match fechado (data|logs), nunca caminho do payload")),
    ("app-quit", RouteAudit::Allowlisted("payload ignorado; encerra o app, mesma sequência do item Sair da bandeja")),
    // -- modules/credentials/mod.rs ------------------------------------------------------------------
    (
        "credentials:test",
        RouteAudit::Validated("owner-gated; provider por match fechado, chave <= 512 sem controle, região Azure só [a-z0-9]; toda URL passa por assert_network_allowed"),
    ),
    // -- modules/desktop.rs -----------------------------------------------------------------------
    ("auto-launch:get", RouteAudit::Allowlisted("sem payload; leitura de booleano de sistema")),
    ("auto-launch:set", RouteAudit::Validated("parse_enable_payload: exige bool, sem coerção")),
    (
        "mika:wizard-prepare-asset",
        RouteAudit::Validated("parse_wizard_asset_payload: assetId por match fechado; voz via is_valid_supertonic_voice/is_valid_kokoro_voice; toda URL é const fixa"),
    ),
    // -- modules/chat/mod.rs ------------------------------------------------------------------------
    ("mika:chat-send", RouteAudit::Validated("send::handle_chat_send: owner-gated; message vazia vira resposta sem I/O, nunca chega a run_turn_llm")),
    ("mika:chat-history", RouteAudit::Allowlisted("owner-gated; sem payload, leitura do HistoryStore")),
    ("mika:chat-clear", RouteAudit::Allowlisted("owner-gated; sem payload, esvazia o HistoryStore da própria janela")),
    ("mika:chat-cancel", RouteAudit::Allowlisted("owner-gated; sem payload, aborta só o turno da própria janela")),
    ("mika:chat-report", RouteAudit::Validated("report::parse_content: owner-gated; content texto não vazio <= 20000, o resto do relatório é lido da config; grava só em <app_data>/reports")),
    (
        "mika:wizard-react",
        RouteAudit::Validated("wizard_react::parse_step: owner-gated; step por match fechado, escolhas lidas da config, nunca do payload"),
    ),
    // -- modules/health.rs --------------------------------------------------------------------------
    ("mika:get-health", RouteAudit::Allowlisted("sem payload; leitura de status de boot")),
    ("mika:prompts-list", RouteAudit::Allowlisted("owner-gated; sem payload de escrita")),
    ("mika:prompts-save", RouteAudit::Validated("is_known_prompt_in: namespace/key contra segmento seguro + arquivo real; PROMPT_OVERRIDE_MAX_CHARS limita o texto")),
    ("mika:prompts-reset", RouteAudit::Validated("mesma allowlist de is_known_prompt_in que mika:prompts-save")),
    // -- modules/legal.rs ----------------------------------------------------------------------------
    (
        "legal:getDocument",
        RouteAudit::Validated("owner-gated; doc por match fechado em DOCUMENTS, o arquivo é resolvido no Rust e o payload nunca carrega caminho"),
    ),
    // -- modules/llm.rs -------------------------------------------------------------------------------
    ("mika:llm-models", RouteAudit::Validated("provider por match fechado; baseUrl explícito passa por privacy::assert_user_endpoint_allowed (SSRF + offline)")),
    ("mika:elevenlabs-voices", RouteAudit::Validated("apiKey obrigatória e não vazia; URL é const fixa")),
    ("llm:getProviderHealth", RouteAudit::Allowlisted("sem payload; lê estado interno do circuit breaker")),
    (
        "llm:resetProviderCircuit",
        RouteAudit::Allowlisted("provider/model são só chave de lookup em HashMap interno; string vazia/arbitrária é chave inexistente, sem efeito externo"),
    ),
    ("llm:getOpenRouterQuota", RouteAudit::Allowlisted("sem payload; usa a API key já salva na config")),
    ("llm:getModelProfile", RouteAudit::Validated("provider por lista fechada; model não vazio e <= 256 chars; URLs/chaves vêm da config e cada busca passa pelo gate de rede de model_profile::fetch")),
    ("llm:getStatus", RouteAudit::Allowlisted("sem payload; sonda o Ollama da config salva via gate de rede (status::probe_ollama)")),
    ("llm:pullModel", RouteAudit::Validated("pull::validate_model_name: [A-Za-z0-9._:/-], <= 128 chars, sem '..' nem prefixo '-'/'/'/'.'; endpoint vem da config e passa pelo gate de rede")),
    ("llm:cancelPull", RouteAudit::Allowlisted("sem payload; só avança a geração do download em curso")),
    // -- modules/e2e_hooks.rs (só existe com MIKA_E2E=1; ver o comentário de topo) -------------------
    (
        "mika:e2e-hook",
        RouteAudit::Validated("op por match fechado; cada handler (publish-event/power-signal/window-set-size/...) valida seus próprios campos, e o módulo só existe com MIKA_E2E=1 + is_owner_surface"),
    ),
    // -- modules/mcp/servers.rs --------------------------------------------------------------------------
    ("mika:mcp-servers-list", RouteAudit::Allowlisted("owner-gated; sem payload; env sai só como chaves (editor::describe), nunca valores")),
    (
        "mika:mcp-server-save",
        RouteAudit::Validated("owner-gated; editor::parse_name (charset/64) + parse_spec (command não vazio, args/env/cwd tipados e com teto, cwd absoluto) + upsert recusa nome repetido/colisão de sanitize"),
    ),
    ("mika:mcp-server-remove", RouteAudit::Validated("owner-gated; editor::parse_name; nome inexistente no mcp.json é erro nomeado")),
    ("mika:mcp-server-test", RouteAudit::Validated("owner-gated; mesmo editor::parse_spec do save + gate de modo offline antes de spawnar; nada é gravado")),
    ("mika:mcp-open-config-dir", RouteAudit::Allowlisted("owner-gated; payload ignorado; abre só a pasta fixa <app_data>/plugins/mika.mcp")),
    // -- modules/plugins/native.rs --------------------------------------------------------------------
    ("mika:plugin-rpc", RouteAudit::Validated("rpc::dispatch: assert_caller_allowed + pluginId/method contra o regex de pluginRpcRequestSchema")),
    // -- modules/settings/mod.rs ------------------------------------------------------------------------
    ("mika:settings-get-all", RouteAudit::Allowlisted("owner-gated; sem payload de escrita")),
    ("mika:settings-set", RouteAudit::Validated("ConfigStore::set: DANGEROUS_KEYS + schema.validate(path, value) antes de gravar")),
    ("mika:settings-ui-prefs", RouteAudit::Allowlisted("sem payload; leitura de locale/tema")),
    ("mika:settings-declarations", RouteAudit::Allowlisted("sem payload; metadado de tela para plugin-view se desenhar")),
    ("mika:plugin-test", RouteAudit::Validated("plugin_test::dispatch valida por plugin antes de rodar o teste de voz/provedor")),
    ("mika:plugin-ui-catalog", RouteAudit::Allowlisted("sem payload; leitura do catálogo de UI por plugin")),
    // -- modules/system/mod.rs --------------------------------------------------------------------------
    ("mika:get-power-state", RouteAudit::Allowlisted("sem payload; leitura de bateria/energia")),
    ("mika:renderer-log", RouteAudit::Validated("validado byte a byte contra desktopRendererLogRequestSchema antes do logger — ver o cabeçalho do arquivo")),
    ("system:getRecentLogs", RouteAudit::Validated("owner-gated (is_owner_surface); sem payload; snapshot do buffer de log")),
    ("system:preventSleep", RouteAudit::Validated("extract_enabled: exige bool")),
    // -- modules/telemetry.rs -----------------------------------------------------------------------------
    ("mika:telemetry-opt-in", RouteAudit::Validated("telemetry_opt_in_with valida o payload antes de gravar o consentimento")),
    ("mika:telemetry-get-status", RouteAudit::Allowlisted("sem payload; leitura de status")),
    ("mika:token-usage-get", RouteAudit::Allowlisted("sem payload; leitura de uso acumulado")),
    ("mika:token-usage-reset", RouteAudit::Allowlisted("sem payload; zera o contador")),
    // -- modules/tools/native.rs ------------------------------------------------------------------------
    ("mika:tools-list", RouteAudit::Allowlisted("sem payload de escrita; listagem filtrada pela superfície do chamador")),
    ("mika:tools-list-by-plugin", RouteAudit::Allowlisted("pluginId vazio ou ausente só amplia o filtro de leitura, nunca concede permissão")),
    ("mika:tool-execute", RouteAudit::Validated("access::assert_tool_execution_allowed + gate de risco da chamada + ToolRegistry::execute_as valida args contra o schema de cada tool")),
    ("tool-approval-response", RouteAudit::Validated("resolve_approval: owner-gated (is_owner_surface, antes de ler o payload); approved ausente é NEGAÇÃO por padrão; scope por match fechado")),
    // -- modules/update.rs ------------------------------------------------------------------------------
    ("mika:update-check", RouteAudit::Allowlisted("payload ignorado; delega para tauri-plugin-updater")),
    ("mika:update-download", RouteAudit::Allowlisted("payload ignorado; baixa a atualização já resolvida por update-check")),
    ("mika:update-quit-and-install", RouteAudit::Allowlisted("payload ignorado; instala o pacote já baixado")),
    ("mika:update-status", RouteAudit::Allowlisted("payload ignorado; leitura do UpdateStatus em memória")),
    // -- modules/voice/mod.rs -----------------------------------------------------------------------------
    ("voice:mic-state", RouteAudit::Validated("extract_enabled: exige bool")),
    ("voice:mic-state-request", RouteAudit::Allowlisted("sem payload; devolve o estado atual do microfone")),
    (
        "mika:voice-transcribe",
        RouteAudit::Validated("parse_payload: audioBase64 não vazio e agora com teto MAX_AUDIO_BASE64_CHARS"),
    ),
    (
        "voice:vad-audio",
        RouteAudit::Validated("parse_samples/parse_amplitude: array numérico com teto MAX_SAMPLES_PER_CALL + amplitude f64 obrigatória"),
    ),
    (
        "mika:voice-silence-command",
        RouteAudit::Validated("silence_command::parse_text: exige 'text' string; comando não reconhecido vira {matched:false}, nunca Err"),
    ),
];

fn lookup(channel: &str) -> Option<RouteAudit> {
    ROUTE_AUDIT.iter().find(|(name, _)| *name == channel).map(|(_, audit)| *audit)
}

/// Todo canal REALMENTE reivindicado em produção (registro nativo + canais de shell) precisa de
/// uma entrada em `ROUTE_AUDIT`. Falta de entrada é o modo de falha que este arquivo existe para
/// pegar: alguém acrescenta um `NativeModule` novo, esquece de vir aqui, e o canal fica sem
/// contabilidade nenhuma — nem `Validated`, nem `Allowlisted` com razão.
#[test]
fn every_active_channel_has_an_audit_entry() {
    let mut missing = Vec::new();

    for channel in crate::bridge::SHELL_CHANNELS {
        if lookup(channel).is_none() {
            missing.push(channel.to_string());
        }
    }

    for module in crate::modules::all() {
        for channel in module.channels() {
            if lookup(channel).is_none() {
                missing.push(format!("{channel} (módulo '{}')", module.name()));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "canal(is) sem entrada em ROUTE_AUDIT (apps/desktop/src-tauri/src/ipc_validation_audit.rs): {missing:?}\n\
         toda rota nova precisa de um veredito Validated(razão) ou Allowlisted(razão) antes de existir."
    );
}

/// Nenhuma entrada pode ficar sem razão de verdade (string vazia) — um `Allowlisted("")` passaria
/// no teste acima sem documentar nada, reabrindo o mesmo buraco que este arquivo fecha.
#[test]
fn every_audit_entry_has_a_non_empty_reason() {
    for (channel, audit) in ROUTE_AUDIT {
        let reason = match audit {
            RouteAudit::Validated(reason) | RouteAudit::Allowlisted(reason) => reason,
        };
        assert!(!reason.trim().is_empty(), "canal '{channel}' tem razão vazia em ROUTE_AUDIT");
    }
}

/// Nenhum canal duplicado — duas entradas para o mesmo canal esconderiam qual veredito vale.
#[test]
fn no_duplicate_channels_in_the_audit() {
    let mut seen = std::collections::HashSet::new();
    for (channel, _) in ROUTE_AUDIT {
        assert!(seen.insert(*channel), "canal '{channel}' duplicado em ROUTE_AUDIT");
    }
}
