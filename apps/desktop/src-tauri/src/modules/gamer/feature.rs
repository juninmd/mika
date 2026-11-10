//! Declaração de settings de `gamer-companion` para o `FeatureRegistry` — port de
//! `gamer-companion.schema.ts::GAMER_COMPANION_SETTINGS` (`defineFeatureSchema`), não da lógica de
//! detecção de jogo/Steam do resto deste diretório. Ver o cabeçalho de `../vision/feature.rs` para
//! por que é um literal JSON calculado à mão em vez de reimplementar `zod-inference.ts`.
//!
//! **`pluginId` é `mika.gamer-companion`, o `manifest.id` de
//! `apps/desktop/src/features/gamer-companion/index.ts`** — mesma convenção de
//! `mcp::MCP_PLUGIN_ID`/`privacy::PRIVACY_PLUGIN_ID`. O `plugin_id` das tools que `register_tools`
//! (`mod.rs`) registra no `ToolRegistry` usa a MESMA constante: a view de plugin (Biblioteca Steam)
//! só executa tool cujo dono bate com o id tirado do label da janela (`tools::access`), e a
//! convenção antiga (`"gamer-companion"`, sem `mika.`) recusava toda chamada.
//!
//! **`momentCooldowns` vira `"type": "object"`, sem detalhar os 6 sub-campos.** O schema TS é
//! `z.object({...}).default(...)`, e `inferFieldMeta` (`zod-inference.ts`) não desce dentro de um
//! `ZodObject` — só retorna `type: 'object'`; a forma interna (limiares por momento) já está
//! portada como namespace de config (`config/schemas/gamer.rs::gamer_companion` → campo
//! `momentCooldowns`, tipo `Map` com o default real), não na declaração de aba.
//!
//! `register_feature()` não recebe `ConfigStore`: mesma razão de `vision::register_feature` — é
//! dado estático, e `enabled: true` espelha `definePlugin`/`registerFeatureWithTools`, que nunca
//! passam um terceiro argumento ao registro.
//!
//! **`topPlayedCache` saiu da lista de campos (era o 13º) na varredura de controles de UI de
//! 2026-09-27.** Nada em `apps/desktop/src-tauri/src`/`apps/desktop/src` lê ou escreve
//! `gamer-companion.topPlayedCache` — era um campo de cache interno sem produtor nem consumidor,
//! exposto como `<input type="text" maxLength=4000>` que o usuário podia editar sem efeito nenhum.
//! O campo de `config/schemas/gamer.rs` continua (o `ConfigStore` mantém chave desconhecida sem
//! erro — ver `AGENTS.md`), isento em `check:config-schema-parity` com o mesmo motivo.

use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::feature_registry::{self, FeatureRegistration};

/// Mesmo `manifest.id` de `apps/desktop/src/features/gamer-companion/index.ts`.
pub const GAMER_COMPANION_PLUGIN_ID: &str = "mika.gamer-companion";

/// Toggle mestre da feature (`gamer-companion.enabled`), lido do `ConfigStore` REAL a cada chamada
/// — nunca cacheado, mesma técnica de `person_catalog::tool::catalog_enabled`/
/// `computer_use::tool::enabled`. Default `true` (config ausente/ilegível não desliga a feature).
pub fn master_enabled(store: &ConfigStore) -> bool {
    store.get("gamer-companion.enabled").and_then(|v| v.as_bool()) != Some(false)
}

/// Resposta padrão de tool quando `gamer-companion.enabled=false` — mesma forma de
/// `person_catalog::tool::catalog_disabled`.
pub fn master_disabled_response() -> Value {
    json!({
        "success": false,
        "error": "The gamer-companion feature is disabled (gamer-companion.enabled=false); no action was taken.",
        "errorCode": "denied",
    })
}

/// Os 12 campos de `GamerCompanionSchema` (`gamer-companion.schema.ts`), na ordem declarada.
fn fields() -> Vec<Value> {
    vec![
        json!({
            "configPath": "gamer-companion.enabled",
            "type": "boolean",
            "label": "settings.gamerCompanion.field.enabled.label",
        }),
        json!({
            "configPath": "gamer-companion.favoriteGames",
            "type": "textarea",
            "label": "settings.gamerCompanion.field.favoriteGames.label",
            "description": "settings.gamerCompanion.field.favoriteGames.description",
            "placeholder": "The Witcher 3, Dark Souls, Hades, Hollow Knight",
            "minLength": 2,
            "maxLength": 480,
        }),
        json!({
            "configPath": "gamer-companion.favoriteAnime",
            "type": "textarea",
            "label": "settings.gamerCompanion.field.favoriteAnime.label",
            "description": "settings.gamerCompanion.field.favoriteAnime.description",
            "placeholder": "Fullmetal Alchemist: Brotherhood, Vinland Saga, Frieren",
            "minLength": 2,
            "maxLength": 480,
        }),
        json!({
            "configPath": "gamer-companion.gameWatcherEnabled",
            "type": "boolean",
            "label": "settings.gamerCompanion.field.gameWatcherEnabled.label",
            "description": "settings.gamerCompanion.field.gameWatcherEnabled.description",
        }),
        json!({
            "configPath": "gamer-companion.gameWatcherIntervalSeconds",
            "type": "number",
            "label": "settings.gamerCompanion.field.gameWatcherIntervalSeconds.label",
            "min": 10,
            "max": 300,
            "inputMode": "decimal",
        }),
        json!({
            "configPath": "gamer-companion.backseatGamingEnabled",
            "type": "boolean",
            "label": "settings.gamerCompanion.field.backseatGamingEnabled.label",
            "description": "settings.gamerCompanion.field.backseatGamingEnabled.description",
        }),
        json!({
            "configPath": "gamer-companion.backseatAggressiveness",
            "type": "number",
            "label": "settings.gamerCompanion.field.backseatAggressiveness.label",
            "description": "settings.gamerCompanion.field.backseatAggressiveness.description",
            "min": 0,
            "max": 100,
            "inputMode": "decimal",
            "visibleWhen": { "configPath": "gamer-companion.backseatGamingEnabled", "equals": true },
        }),
        json!({
            "configPath": "gamer-companion.momentCooldowns",
            "type": "object",
            "label": "settings.gamerCompanion.field.momentCooldowns.label",
            "description": "settings.gamerCompanion.field.momentCooldowns.description",
        }),
        json!({
            "configPath": "gamer-companion.sessionSummaryEnabled",
            "type": "boolean",
            "label": "settings.gamerCompanion.field.sessionSummaryEnabled.label",
            "description": "settings.gamerCompanion.field.sessionSummaryEnabled.description",
        }),
        json!({
            "configPath": "gamer-companion.steamPath",
            "type": "text",
            "label": "settings.gamerCompanion.field.steamPath.label",
            "description": "settings.gamerCompanion.field.steamPath.description",
            "placeholder": "C:\\Program Files (x86)\\Steam",
            "maxLength": 260,
        }),
        json!({
            "configPath": "gamer-companion.steamApiKey",
            "type": "password",
            "label": "settings.gamerCompanion.field.steamApiKey.label",
            "sensitive": true,
            "description": "settings.gamerCompanion.field.steamApiKey.description",
            "placeholder": "XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
            "learnMoreUrl": "https://steamcommunity.com/dev/apikey",
            "maxLength": 512,
        }),
        json!({
            "configPath": "gamer-companion.steamId64",
            "type": "text",
            "label": "settings.gamerCompanion.field.steamId64.label",
            "description": "settings.gamerCompanion.field.steamId64.description",
            "placeholder": "76561198XXXXXXXXX",
            "maxLength": 20,
        }),
    ]
}

/// `GAMER_COMPANION_SETTINGS` (`gamer-companion.schema.ts`), sem `pluginId`/`isCore` — o registro
/// injeta os dois. `enabledConfigPath` é inferido de `gamer-companion.enabled` (primeiro grupo com
/// campo `enabled`), mesma regra de `inferEnabledConfigPath` do lado TS.
pub fn declaration() -> Value {
    json!({
        "tabId": "gamer-companion",
        "tabLabel": "settings.gamerCompanion.tab.label",
        "tabIcon": "Gamepad2",
        "tabGroup": "settings.gamerCompanion.tab.group",
        "tabDescription": "settings.gamerCompanion.tab.description",
        "accentColor": "violet",
        "component": "GamerCompanionPanel",
        "enabledConfigPath": "gamer-companion.enabled",
        "groups": [{
            "title": "settings.gamerCompanion.group.title",
            "component": "GamerCompanionPanel",
            "groupPrefix": "gamer-companion",
            "columns": 2,
            "fields": fields(),
        }],
    })
}

/// As 3 `PluginViewDeclaration` de `GamerCompanionFeature.views` (`gamer-companion.feature.ts`),
/// sem `pluginId` — o registro injeta (`inject_plugin_id`, ver `feature_registry/merge.rs`). Título
/// e descrição são texto pt-BR LITERAL no TS (não chave i18n), copiados como estão: `mika:plugin-
/// ui-catalog` nunca passa por `localize_feature_settings` (só `mika:settings-declarations` passa —
/// ver o cabeçalho de `../plugin_ui.rs`).
fn views() -> Vec<Value> {
    vec![
        json!({
            "id": "gamer-companion.hud",
            "title": "HUD do Jogo",
            "component": "gameHud",
            "description": "Sessão atual, tempo de jogo, conquistas recentes e os momentos marcantes da partida.",
            "defaultWindow": { "width": 980, "height": 700, "minWidth": 760, "minHeight": 520 },
        }),
        json!({
            "id": "gamer-companion.steam-library",
            "title": "Biblioteca Steam",
            "component": "steamLibrary",
            "description": "Jogos instalados e mais jogados, com tempo acumulado e atalho para abrir pela Steam.",
            "defaultWindow": { "width": 1100, "height": 760, "minWidth": 860, "minHeight": 560 },
        }),
        json!({
            "id": "gamer-companion.mini-games",
            "title": "Mini Games",
            "component": "miniGames",
            "description": "Jogos rápidos para jogar com a Mika.",
            "defaultWindow": { "width": 900, "height": 680, "minWidth": 700, "minHeight": 520 },
        }),
    ]
}

/// Registra a aba `gamer-companion` no `FeatureRegistry` compartilhado sob `mika.gamer-companion`
/// — mesmo `pluginId` de `manifest.id`. Chame UMA VEZ no boot.
pub fn register_feature() {
    feature_registry::shared().register(
        GAMER_COMPANION_PLUGIN_ID,
        FeatureRegistration {
            enabled: true,
            settings: vec![declaration()],
            views: views(),
            ..Default::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_has_the_expected_tab_shape() {
        let d = declaration();
        assert_eq!(d["tabId"], json!("gamer-companion"));
        assert_eq!(d["enabledConfigPath"], json!("gamer-companion.enabled"));
        assert_eq!(d["accentColor"], json!("violet"));
        assert_eq!(d["groups"].as_array().unwrap().len(), 1);
        assert_eq!(d["groups"][0]["fields"].as_array().unwrap().len(), 12);
    }

    /// Toda `configPath` é prefixada por `gamer-companion.` — um campo sem o prefixo grava fora do
    /// namespace da feature, silenciosamente.
    #[test]
    fn every_field_config_path_is_under_the_gamer_companion_namespace() {
        for field in declaration()["groups"][0]["fields"].as_array().unwrap() {
            let path = field["configPath"].as_str().unwrap();
            assert!(path.starts_with("gamer-companion."), "configPath fora do namespace: {path}");
        }
    }

    /// `topPlayedCache` não pode voltar a aparecer como campo — nada no backend o lê/escreve (ver
    /// o cabeçalho do arquivo); se um novo consumidor real nascer, a UI volta com o campo E este
    /// teste é quem avisa que o pin ficou desatualizado, não este teste sozinho autorizando o campo.
    #[test]
    fn top_played_cache_is_not_a_field_anymore() {
        assert!(
            declaration()["groups"][0]["fields"]
                .as_array()
                .unwrap()
                .iter()
                .all(|f| f["configPath"] != json!("gamer-companion.topPlayedCache")),
            "topPlayedCache voltou à UI sem um consumidor real documentado"
        );
    }

    /// Toggle mestre lido do `ConfigStore` REAL — nenhum caminho para `false` explícito escapa.
    #[test]
    fn master_enabled_defaults_to_true_when_config_is_absent() {
        let (store, _dir) = crate::modules::gamer::test_support::empty_config_store();
        assert!(master_enabled(&store));
    }

    #[test]
    fn master_enabled_is_false_only_when_explicitly_disabled() {
        let (store, _dir) = crate::modules::gamer::test_support::empty_config_store();
        store.set("gamer-companion.enabled", json!(false)).unwrap();
        assert!(!master_enabled(&store));
        store.set("gamer-companion.enabled", json!(true)).unwrap();
        assert!(master_enabled(&store));
    }

    #[test]
    fn register_feature_publishes_the_gamer_companion_tab_under_the_plugin_manifest_id() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&GAMER_COMPANION_PLUGIN_ID.into())
            .expect("mika.gamer-companion registrado");
        assert!(registered.enabled);
        assert_eq!(registered.settings[0]["tabId"], json!("gamer-companion"));
    }

    /// `mika:plugin-ui-catalog` (`get_ui_catalog`) lê `views`, não `settings` — sem esta asserção
    /// as 3 views do HUD/biblioteca Steam/mini games somem do catálogo em silêncio (elas não têm
    /// `tabId`, então a dedupe de `get_all_settings` nunca acusaria a ausência).
    #[test]
    fn register_feature_publishes_the_three_views_too() {
        register_feature();
        let registered = feature_registry::shared()
            .get(&GAMER_COMPANION_PLUGIN_ID.into())
            .expect("mika.gamer-companion registrado");
        let ids: Vec<&str> = registered.views.iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert_eq!(
            ids,
            vec!["gamer-companion.hud", "gamer-companion.steam-library", "gamer-companion.mini-games"]
        );
    }
}
