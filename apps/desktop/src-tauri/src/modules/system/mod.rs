//! Módulo `system` em Rust — port dos 4 canais de sistema do sidecar Node:
//! `mika:get-power-state`, `mika:renderer-log`, `system:getRecentLogs`, `system:preventSleep`.
//!
//! Ver `apps/desktop/src/main/ipc/health.ts` (`getPowerState`, `rendererLog`) e
//! `apps/desktop/src/main/ipc/system-metrics.ts` (`getRecentLogs`, `preventSleep`) para o
//! comportamento que este módulo reproduz. Bateria/energia reúsam `crate::power`
//! (`starship-battery`/`keepawake`, já cross-platform por crate — nenhum `#[cfg(windows)]` aqui).
//!
//! `mika:renderer-log` recebe TEXTO DO RENDERER, inclusive de uma plugin-view sob controle de
//! terceiro — este repositório já teve dado do usuário vazando para um painel que não deveria
//! mostrá-lo. Por isso a entrada é tratada como hostil: validada byte a byte contra os mesmos
//! limites de `desktopRendererLogRequestSchema` (packages/sdk/src/contracts/desktop-ipc-plugin-schemas.ts)
//! antes de tocar o logger, nunca repassada crua.
//!
//! Virou diretório em 2026-09-22 para caber [`tools`]: a tool `get_system_info`
//! (`packages/sdk/src/tools/tool-registry.ts::ToolRegistryService.init()`), a única do catálogo TS
//! registrada FORA de qualquer feature — por isso mora aqui, no módulo `system`, e não em
//! `features/`/`gamer/`. Config-livre (não lê `ConfigStore`), registrada como efeito colateral de
//! [`modules()`], mesmo padrão de `gamer::register_tools`/`brain::tools::register_config_free_tools`.

use std::sync::Mutex;

mod human_log;
pub mod log_buffer;
pub mod log_files;
pub mod log_trace;
pub mod log_redact;
pub mod log_stream;
mod tools;
pub mod governor;

use async_trait::async_trait;
use log::Level;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::native::NativeModule;
use crate::power;
use crate::protocol::{InvokeContext, WindowType};

const CHANNELS: [&str; 4] = [
    "mika:get-power-state",
    "mika:renderer-log",
    "system:getRecentLogs",
    "system:preventSleep",
];

// Espelham `desktopRendererLogRequestSchema`. Divergir aqui aceita (ou rejeita) entrada que o
// contrato TypeScript recusaria (ou aceitaria) do lado Node — os dois lados falam do mesmo canal.
const RENDERER_LOG_SCOPE_MAX_CHARS: usize = 64;
const RENDERER_LOG_MSG_MAX_CHARS: usize = 300;

struct SystemModule {
    // `invoke` recebe `&self` compartilhado entre chamadas — substitui o `let sleepBlockerId`
    // fechado por closure em system-metrics.ts, que não sobrevive à travessia Rust.
    sleep_blocker_id: Mutex<Option<u64>>,
}

#[async_trait]
impl NativeModule for SystemModule {
    fn name(&self) -> &'static str {
        "system"
    }

    fn channels(&self) -> &'static [&'static str] {
        &CHANNELS
    }

    async fn invoke(
        &self,
        _app: &AppHandle,
        ctx: &InvokeContext,
        channel: &str,
        payload: Value,
    ) -> Result<Value, String> {
        match channel {
            "mika:get-power-state" => Ok(get_power_state()),
            "mika:renderer-log" => renderer_log(ctx, &payload),
            "system:getRecentLogs" => get_recent_logs(ctx),
            "system:preventSleep" => prevent_sleep(&self.sleep_blocker_id, &payload),
            // `channels()` é a fonte da verdade; chegar aqui sem braço é bug de programação
            // NESTE módulo (canal declarado e esquecido), não payload hostil — erro explícito.
            other => Err(format!("canal '{other}' declarado por 'system' mas não tratado")),
        }
    }
}

/// Bateria + o orçamento do governador de energia (perfil, estado, teto de fps) — o renderer lê os
/// dois juntos para decidir a qualidade do avatar e a leitura ao vivo da aba Resumo. Só a bateria:
/// `power::snapshot` também mede ócio e falha sem servidor X, o que derrubava a rota inteira.
/// Erro de enumeração vira `false`, mesmo contrato de `health.ts` (`?? false`).
fn get_power_state() -> Value {
    let mut state = governor::snapshot();
    state["onBatteryPower"] = json!(power::on_battery_power().unwrap_or(false));
    state
}

/// `system:getRecentLogs` no Node lia um buffer em memória (`getRecentLogs()` de `@mika/sdk`)
/// alimentado por tudo que passa por `mikaLogger`. O equivalente Rust é [`log_buffer`]: um target
/// extra de `tauri_plugin_log` (`lib.rs`) alimenta o ring buffer a cada `log::log!` do processo, e
/// esta função devolve o snapshot atual — a carga inicial do painel; o fluxo contínuo é
/// [`log_stream::spawn`].
///
/// **Só superfície do dono.** O buffer guarda tudo que o processo logou (caminhos, nomes de
/// provedor, erros com trechos de resposta): uma `plugin-view` de terceiro não lê isso.
fn get_recent_logs(ctx: &InvokeContext) -> Result<Value, String> {
    if !crate::shell_actions::is_owner_surface(ctx.window_type) {
        return Err("system:getRecentLogs negado: janela não é superfície do dono".to_string());
    }
    let logs: Vec<Value> = log_buffer::snapshot()
        .into_iter()
        .map(|record| serde_json::to_value(record).unwrap_or(Value::Null))
        .collect();
    Ok(json!({ "logs": logs }))
}

fn extract_enabled(payload: &Value) -> Result<bool, String> {
    payload["enabled"]
        .as_bool()
        .ok_or_else(|| "payload deve ter 'enabled' booleano".to_string())
}

fn prevent_sleep(blocker_id: &Mutex<Option<u64>>, payload: &Value) -> Result<Value, String> {
    let enabled = extract_enabled(payload)?;

    let mut guard = blocker_id
        .lock()
        .map_err(|_| "lock do bloqueador de sono envenenado".to_string())?;

    if enabled && guard.is_none() {
        *guard = Some(power::start_blocker("prevent-app-suspension")?);
    } else if !enabled {
        if let Some(id) = guard.take() {
            power::stop_blocker(id)?;
        }
    }

    Ok(json!({ "ok": true }))
}

fn parse_level(raw: &str) -> Result<Level, String> {
    match raw {
        "debug" => Ok(Level::Debug),
        "info" => Ok(Level::Info),
        "warn" => Ok(Level::Warn),
        "error" => Ok(Level::Error),
        other => Err(format!(
            "'level' inválido: '{other}' — precisa ser debug, info, warn ou error"
        )),
    }
}

// Comprimento em CARACTERES, não bytes: `msg`/`scope` hostis podem ter multi-byte (emoji,
// acentos), e um `.len()` em bytes rejeitaria UTF-8 válido bem antes do limite do contrato.
fn char_len_in_bounds(value: &str, min: usize, max: usize) -> bool {
    let len = value.chars().count();
    len >= min && len <= max
}

fn renderer_log(ctx: &InvokeContext, payload: &Value) -> Result<Value, String> {
    let level_raw = payload["level"].as_str().ok_or("'level' ausente")?;
    let level = parse_level(level_raw)?;

    let scope = payload["scope"].as_str().ok_or("'scope' ausente")?;
    if !char_len_in_bounds(scope, 1, RENDERER_LOG_SCOPE_MAX_CHARS) {
        return Err(format!(
            "'scope' fora do limite (1..{RENDERER_LOG_SCOPE_MAX_CHARS} caracteres)"
        ));
    }

    let msg = payload["msg"].as_str().ok_or("'msg' ausente")?;
    if !char_len_in_bounds(msg, 1, RENDERER_LOG_MSG_MAX_CHARS) {
        return Err(format!(
            "'msg' fora do limite (1..{RENDERER_LOG_MSG_MAX_CHARS} caracteres)"
        ));
    }

    // `meta` é opcional e livre (`record<string, unknown>` no contrato): só recusa o tipo
    // errado, sem validar o conteúdo interno — mesmo grau de checagem que o Zod do lado TS.
    let meta = &payload["meta"];
    if !meta.is_null() && !meta.is_object() {
        return Err("'meta' precisa ser objeto".to_string());
    }

    let scope = attribute_scope(scope, ctx);

    // Só chega aqui depois de truncado pelos limites acima, mesmo vindo de uma plugin-view
    // hostil. Vai para stderr/arquivo do shell (`tauri_plugin_log`), não para o painel de Logs
    // do renderer — ver a lacuna documentada em `get_recent_logs`.
    log::log!(target: "renderer", level, "[{scope}] {msg}");

    Ok(json!({ "ok": true }))
}

/// Espelha `attributeScope` em `apps/desktop/src/main/ipc/health.ts`: uma plugin-view compartilha
/// o mesmo preload das janelas core e podia forjar um `scope` de origem que não é ela — o
/// contexto vem do label da janela que o Tauri resolve (`ctx.window_type`/`ctx.plugin_id`), não do
/// payload, então o prefixo não é forjável pelo renderer.
fn attribute_scope(scope: &str, ctx: &InvokeContext) -> String {
    if ctx.window_type != WindowType::PluginView {
        return scope.to_string();
    }
    format!(
        "plugin:{}/{}",
        ctx.plugin_id.as_deref().unwrap_or("desconhecido"),
        scope
    )
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    tools::register();
    vec![Box::new(SystemModule {
        sleep_blocker_id: Mutex::new(None),
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(window_type: WindowType, plugin_id: Option<&str>) -> InvokeContext {
        InvokeContext {
            window_type,
            window_label: "test".to_string(),
            plugin_id: plugin_id.map(str::to_string),
        }
    }

    #[test]
    fn recent_logs_are_for_owner_surfaces_only() {
        for window in [WindowType::Main, WindowType::Settings, WindowType::Wizard] {
            assert!(get_recent_logs(&ctx(window, None)).is_ok(), "{window:?}");
        }
        for window in [WindowType::PluginView, WindowType::Unknown] {
            assert!(get_recent_logs(&ctx(window, Some("mika.evil"))).is_err(), "{window:?}");
        }
    }

    #[test]
    fn claims_exactly_the_four_channels() {
        let module = SystemModule {
            sleep_blocker_id: Mutex::new(None),
        };
        assert_eq!(
            module.channels(),
            &[
                "mika:get-power-state",
                "mika:renderer-log",
                "system:getRecentLogs",
                "system:preventSleep",
            ]
        );
    }

    /// Mesmo padrão de `bridge.rs::shell_channels_match_typescript_contract`: um literal errado
    /// aqui não quebra a compilação, só faz o canal nunca ser reivindicado e cair de volta pro
    /// sidecar em silêncio.
    #[test]
    fn channels_match_typescript_contract() {
        let channels_contract =
            include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");
        let routes_contract =
            include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc.ts");

        for channel in ["mika:get-power-state", "mika:renderer-log"] {
            assert!(
                channels_contract.contains(&format!("'{channel}'")),
                "canal {channel} não existe em desktop-ipc-channels.ts"
            );
        }
        for channel in ["system:getRecentLogs", "system:preventSleep"] {
            assert!(
                routes_contract.contains(&format!("'{channel}'")),
                "canal {channel} não existe em desktop-ipc.ts"
            );
        }
    }

    /// O singleton de `log_buffer` só é populado pela fiação de produção (`lib.rs`); nenhum teste
    /// deste binário escreve nele, então o snapshot fica vazio aqui — não é mais um valor
    /// hardcoded como antes de `log_buffer` existir, é o estado real de um processo de teste que
    /// nunca logou nada.
    #[test]
    fn get_recent_logs_returns_empty_list_when_nothing_was_logged_yet() {
        assert_eq!(get_recent_logs(&ctx(WindowType::Main, None)).unwrap(), json!({ "logs": [] }));
    }

    #[test]
    fn prevent_sleep_rejects_missing_enabled() {
        assert!(extract_enabled(&json!({})).is_err());
    }

    #[test]
    fn prevent_sleep_rejects_non_bool_enabled() {
        assert!(extract_enabled(&json!({ "enabled": "yes" })).is_err());
    }

    #[test]
    fn prevent_sleep_accepts_bool_enabled() {
        assert_eq!(extract_enabled(&json!({ "enabled": true })).unwrap(), true);
        assert_eq!(extract_enabled(&json!({ "enabled": false })).unwrap(), false);
    }

    #[test]
    fn renderer_log_rejects_invalid_level() {
        let payload = json!({ "level": "trace", "scope": "x", "msg": "hi" });
        let err = renderer_log(&ctx(WindowType::Main, None), &payload).unwrap_err();
        assert!(err.contains("level"));
    }

    #[test]
    fn renderer_log_rejects_missing_level() {
        let payload = json!({ "scope": "x", "msg": "hi" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_rejects_empty_scope() {
        let payload = json!({ "level": "info", "scope": "", "msg": "hi" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_rejects_scope_over_limit() {
        let long_scope = "a".repeat(RENDERER_LOG_SCOPE_MAX_CHARS + 1);
        let payload = json!({ "level": "info", "scope": long_scope, "msg": "hi" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_accepts_scope_at_limit() {
        let scope_at_limit = "a".repeat(RENDERER_LOG_SCOPE_MAX_CHARS);
        let payload = json!({ "level": "info", "scope": scope_at_limit, "msg": "hi" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_ok());
    }

    #[test]
    fn renderer_log_rejects_missing_scope() {
        let payload = json!({ "level": "info", "msg": "hi" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_rejects_empty_msg() {
        let payload = json!({ "level": "info", "scope": "x", "msg": "" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_rejects_msg_over_limit() {
        let long_msg = "a".repeat(RENDERER_LOG_MSG_MAX_CHARS + 1);
        let payload = json!({ "level": "info", "scope": "x", "msg": long_msg });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_rejects_missing_msg() {
        let payload = json!({ "level": "info", "scope": "x" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    /// `msg`/`scope` hostis podem ter multi-byte (emoji): o limite é em caracteres, não bytes —
    /// 300 emojis (4 bytes cada em UTF-8) têm de passar.
    #[test]
    fn renderer_log_counts_unicode_chars_not_bytes() {
        let msg = "🎮".repeat(RENDERER_LOG_MSG_MAX_CHARS);
        let payload = json!({ "level": "info", "scope": "x", "msg": msg });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_ok());
    }

    #[test]
    fn renderer_log_accepts_missing_meta() {
        let payload = json!({ "level": "info", "scope": "x", "msg": "hi" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_ok());
    }

    #[test]
    fn renderer_log_accepts_object_meta() {
        let payload = json!({ "level": "info", "scope": "x", "msg": "hi", "meta": { "k": "v" } });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_ok());
    }

    #[test]
    fn renderer_log_rejects_non_object_meta() {
        let payload = json!({ "level": "info", "scope": "x", "msg": "hi", "meta": "not an object" });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn renderer_log_rejects_array_meta() {
        let payload = json!({ "level": "info", "scope": "x", "msg": "hi", "meta": [1, 2] });
        assert!(renderer_log(&ctx(WindowType::Main, None), &payload).is_err());
    }

    #[test]
    fn attribute_scope_passes_through_for_owner_surfaces() {
        assert_eq!(attribute_scope("boot", &ctx(WindowType::Main, None)), "boot");
        assert_eq!(attribute_scope("boot", &ctx(WindowType::Settings, None)), "boot");
        assert_eq!(attribute_scope("boot", &ctx(WindowType::Wizard, None)), "boot");
    }

    #[test]
    fn attribute_scope_prefixes_plugin_views_with_plugin_id() {
        let plugin_ctx = ctx(WindowType::PluginView, Some("acme"));
        assert_eq!(attribute_scope("boot", &plugin_ctx), "plugin:acme/boot");
    }

    /// Uma plugin-view sem `pluginId` resolvido não pode virar um `scope` indistinguível de uma
    /// janela core — cai num rótulo explícito, nunca no `scope` cru do payload.
    #[test]
    fn attribute_scope_falls_back_when_plugin_id_missing() {
        let plugin_ctx = ctx(WindowType::PluginView, None);
        assert_eq!(attribute_scope("boot", &plugin_ctx), "plugin:desconhecido/boot");
    }

    /// Uma plugin-view não consegue forjar o prefixo de outro plugin escrevendo isso no
    /// `scope` do payload — o prefixo real vem do `ctx` (label da janela), não do texto.
    #[test]
    fn attribute_scope_does_not_trust_forged_scope_text() {
        let plugin_ctx = ctx(WindowType::PluginView, Some("acme"));
        let forged_scope = "plugin:other-plugin/boot";
        assert_eq!(
            attribute_scope(forged_scope, &plugin_ctx),
            "plugin:acme/plugin:other-plugin/boot"
        );
    }
}
