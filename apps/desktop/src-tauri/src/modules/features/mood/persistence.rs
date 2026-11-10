//! Persistência de afinidade via `ConfigStore` — fecha a dívida do item 2 do cabeçalho de `mod.rs`.
//!
//! Com `register_config_bound_tools` (`modules/mod.rs`, coordenação `ebb5d7eb`), a tool `mood`
//! ganha um SEGUNDO registro, feito DEPOIS que o Tauri monta o app e o `ConfigStore` existe. Até
//! aqui isso era bloqueado porque `ToolHandler::call` não recebe `AppHandle` e `features::modules()`
//! roda antes dele existir — o mesmo bloqueio que `desktop`/`vision` documentam. Não é mais o caso.
//!
//! Port de `packages/mood/src/ports.ts::AffinityScoreStore` + a leitura/escrita em
//! `apps/desktop/src/features/mood/index.ts::activate` (`ports.score.read`/`write`): o placar vive
//! em `character.affinityScore` (-100..100, schema já portado em `config/schemas/character.rs`),
//! COMPARTILHADO com a tool `avatar` (`action=affinity_delta`, `features/character`) e o rastreador
//! de emoção do brain (fora do meu escopo) — por isso [`PersistentAffinity`] lê FRESCO do store a
//! cada chamada, nunca um valor cacheado em memória, para não apagar o que os outros escritores
//! gravaram (mesmo comentário de `AffinityService` no TS: "três escritores independentes disputam
//! `character.affinityScore`").
//!
//! `action=update`/`action=register_interaction` TAMBÉM publicam `EventKey::MoodChanged`/
//! `MoodAffinityLevelUp` no barramento de processo (`crate::events`) — a chave passou a existir
//! nesta MESMA rodada (frente paralela de eventos, 2026-09-21c) e fecha a outra metade da dívida
//! original ("`mood:changed`/`mood:affinity-level-up` não são publicados"). `register_tools`
//! (`tool.rs`, primeira passada sem config) publica os MESMOS eventos — quem estiver assinando não
//! depende de `register_config_bound_tools` ter sucesso.
//!
//! ## Fechado NESTA rodada: `mood.affinityGainRate` configurável, aba `mood`, timer de decaimento
//!
//! `config/schemas/mood.rs` agora existe. [`PersistentAffinity::gain_rate`] lê
//! `mood.affinityGainRate` FRESCO a cada `register_interaction` (mesmo padrão de
//! [`PersistentAffinity::read_percent`] para o placar). [`register`] também declara a aba `mood`
//! em `crate::feature_registry::shared()` (mesmo padrão de `user::register`/`character::register`)
//! e dispara [`super::boot::spawn_decay_timer`] — o `setInterval` de decaimento que faltava.
//!
//! **Valência/excitação (humor) continuam em memória de processo**, não em `character.*` — o TS
//! TAMBÉM nunca persistiu essa metade (só a afinidade vai para o `ConfigStore`); não é uma
//! divergência nova.
//!
//! `get`/`register_interaction` passam a devolver `"affinityPersisted": true` — o oposto do que
//! `engine.rs` devolve sem este registro, para que a paridade real (ou a falta dela) nunca seja
//! lida por engano no `data` da tool.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::store::ConfigStore;
use crate::events::{bus, EventKey};
use crate::modules::tools::{ToolDefinition, ToolHandler};

use super::engine::engine;
use super::math::{affinity_label_for, affinity_level_for, clamp_percent, DEFAULT_GAIN_RATE};

/// Mesmo caminho que `apps/desktop/src/features/mood/index.ts::activate` lê/escreve.
const AFFINITY_CONFIG_PATH: &str = "character.affinityScore";
/// `config/schemas/mood.rs` — `mood.schema.ts::affinityGainRate`.
const GAIN_RATE_CONFIG_PATH: &str = "mood.affinityGainRate";

/// `character.affinityScore` é `-100..100` (ver `config/schemas/character.rs`); a afinidade do
/// `mood`/`affinity` é `0..100` — a MESMA fórmula do TS (`ports: { score: { read, write } }`), não
/// uma nova.
fn raw_to_percent(raw: f64) -> f64 {
    clamp_percent((raw + 100.0) / 2.0)
}

fn percent_to_raw(percent: f64) -> f64 {
    (percent * 2.0 - 100.0).clamp(-100.0, 100.0)
}

pub(super) struct PersistentAffinity {
    store: Arc<ConfigStore>,
}

impl PersistentAffinity {
    pub(super) fn new(store: Arc<ConfigStore>) -> Self {
        Self { store }
    }

    /// Lê FRESCO — três escritores independentes disputam este caminho (ver cabeçalho do módulo).
    fn read_percent(&self) -> f64 {
        let raw = self.store.get(AFFINITY_CONFIG_PATH).and_then(|v| v.as_f64()).unwrap_or(0.0);
        raw_to_percent(raw)
    }

    /// Lido FRESCO a cada `add_interaction` — configurável em runtime (`config/schemas/mood.rs`),
    /// então um valor cacheado no construtor ficaria desatualizado se o usuário mudar o slider.
    fn gain_rate(&self) -> f64 {
        self.store.get(GAIN_RATE_CONFIG_PATH).and_then(|v| v.as_f64()).unwrap_or(DEFAULT_GAIN_RATE)
    }

    fn write_percent(&self, percent: f64) -> Result<(), String> {
        self.store.set(AFFINITY_CONFIG_PATH, json!(percent_to_raw(percent)))
    }

    pub(super) fn snapshot(&self) -> Value {
        let score = self.read_percent();
        json!({ "level": affinity_level_for(score), "label": affinity_label_for(score), "score": score })
    }

    /// Espelha `AffinityService.addInteraction` com `readExternalScore` presente: relê antes de
    /// somar o ganho, e grava de volta — devolve `(snapshot, level_up)`.
    pub(super) fn add_interaction(&self, weight: f64) -> Result<(Value, bool), String> {
        let previous_score = self.read_percent();
        let previous_level = affinity_level_for(previous_score);
        let next_score = clamp_percent(previous_score + self.gain_rate() * weight);
        self.write_percent(next_score)?;
        let level_up = affinity_level_for(next_score) > previous_level;
        Ok((
            json!({
                "level": affinity_level_for(next_score),
                "label": affinity_label_for(next_score),
                "score": next_score,
            }),
            level_up,
        ))
    }
}

struct PersistentMoodToolHandler {
    affinity: PersistentAffinity,
}

#[async_trait]
impl ToolHandler for PersistentMoodToolHandler {
    async fn call(&self, args: Value) -> Result<Value, String> {
        let action = args.get("action").and_then(Value::as_str).unwrap_or_default();
        match action {
            "get" => Ok(json!({
                "success": true,
                "data": { "mood": engine().mood_snapshot(), "affinity": self.affinity.snapshot() },
                "affinityPersisted": true,
            })),
            "update" => {
                let valence = args.get("valence").and_then(Value::as_f64);
                let arousal = args.get("arousal").and_then(Value::as_f64);
                match (valence, arousal) {
                    (Some(v), Some(a)) => {
                        let result = engine().update(v, a);
                        bus().publish(EventKey::MoodChanged, result["data"].clone());
                        Ok(result)
                    }
                    _ => Err("action='update' requires both valence and arousal.".into()),
                }
            }
            "register_interaction" => {
                let weight = args.get("weight").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.5, 5.0);
                let reason = args.get("reason").and_then(Value::as_str);
                let (mut data, level_up) = self.affinity.add_interaction(weight)?;
                if level_up {
                    bus().publish(
                        EventKey::MoodAffinityLevelUp,
                        json!({ "level": data["level"], "label": data["label"], "score": data["score"] }),
                    );
                }
                if let Some(obj) = data.as_object_mut() {
                    if let Some(reason) = reason {
                        obj.insert("reason".into(), json!(reason));
                    }
                    obj.insert("levelUp".into(), json!(level_up));
                }
                Ok(json!({ "success": true, "data": data, "affinityPersisted": true }))
            }
            "decay" => {
                let factor = args.get("factor").and_then(Value::as_f64).unwrap_or(0.5).clamp(0.1, 1.0);
                Ok(engine().decay(factor))
            }
            other => Err(format!("mood: ação desconhecida \"{other}\"")),
        }
    }
}

fn tool_definition() -> ToolDefinition {
    super::tool::definition()
}

/// Chamado por `modules::register_config_bound_tools(app)` — SUBSTITUI o registro sem config feito
/// por `register_tools()` (mesmo nome normalizado, `ToolRegistry::register` sobrescreve — ver o
/// cabeçalho dele): `get`/`register_interaction` passam a ler/escrever `character.affinityScore`
/// de verdade; `update`/`decay` continuam sobre o motor de humor em memória (inalterado — ver o
/// item 1 do cabeçalho de `mod.rs`). Também declara a aba `mood`, dispara o timer de decaimento, e
/// registra os 4 métodos de `mika:plugin-rpc` (`super::rpc`) — mesmo `ConfigStore`, uma chamada só.
/// The group names the renderer's `MoodPanel`; without it the tab showed "painel manual ausente".
fn mood_settings(schema: &crate::config::registry::SchemaRegistry) -> Option<Value> {
    let mut settings = super::super::settings_declaration::declare_namespace(
        schema,
        "mood",
        "mood",
        "settings.mood.tab.label",
        "Heart",
        "settings.mood.tab.label",
    )?;
    settings["groups"][0]["component"] = Value::from("MoodPanel");
    settings["tabDescription"] = Value::from("settings.mood.tab.description");
    settings["enabledConfigPath"] = Value::from("mood.enabled");
    Some(settings)
}

pub fn register(store: Arc<ConfigStore>) {
    crate::modules::tools::registry().register(
        tool_definition(),
        Arc::new(PersistentMoodToolHandler { affinity: PersistentAffinity::new(Arc::clone(&store)) }),
    );

    if let Some(settings) = mood_settings(store.schema()) {
        crate::feature_registry::shared().register(
            "mika.mood",
            crate::feature_registry::FeatureRegistration { enabled: true, settings: vec![settings], ..Default::default() },
        );
    } else {
        log::warn!("mood: namespace 'mood' ausente do SchemaRegistry — settings não declaradas");
    }

    super::boot::spawn_decay_timer(Arc::clone(&store));
    super::turn_mood::spawn_turn_mood_listener(Arc::clone(&store));
    super::needs::register(Arc::clone(&store));
    super::rpc::register(store);
    super::plugin_test::register();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::registry::{Namespace, SchemaRegistry};
    use crate::config::schema::float_range;
    use std::path::PathBuf;

    #[test]
    fn the_mood_tab_renders_a_panel_the_renderer_registers() {
        let registry = include_str!("../../../../../src/renderer/src/components/settings/feature-components.tsx");
        assert!(registry.contains("MoodPanel: (props)"));
        let settings = mood_settings(&crate::config::schemas::registry()).expect("mood namespace declared");
        assert_eq!(settings["groups"][0]["component"], "MoodPanel");
    }

    // Without these the UI guessed `<tabId>.enabled` and showed a generic description.
    #[test]
    fn the_mood_tab_declares_its_master_switch_and_a_description_the_locales_carry() {
        let settings = mood_settings(&crate::config::schemas::registry()).expect("mood namespace declared");
        assert_eq!(settings["enabledConfigPath"], "mood.enabled");
        assert!(crate::config::schemas::registry().validate("mood.enabled", &serde_json::json!(false)).is_ok());
        let description = settings["tabDescription"].as_str().expect("tabDescription");
        assert_ne!(crate::modules::settings::t(description, "pt-BR"), description, "{description} missing from pt-BR");
    }

    /// Mesmo padrão de `vision.rs`/`privacy.rs`: diretório isolado por teste.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-mood-persistence-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store() -> (Arc<ConfigStore>, TempDir) {
        let dir = TempDir::new();
        let schema = SchemaRegistry::new(vec![
            Namespace { name: "character", fields: vec![float_range("affinityScore", 0.0, -100.0, 100.0)] },
            Namespace { name: "mood", fields: vec![float_range("affinityGainRate", 1.0, 0.0, 10.0)] },
        ]);
        (Arc::new(ConfigStore::open(&dir.0, schema).unwrap()), dir)
    }

    #[test]
    fn raw_percent_conversion_round_trips_like_the_typescript_ports() {
        assert_eq!(raw_to_percent(0.0), 50.0);
        assert_eq!(raw_to_percent(-100.0), 0.0);
        assert_eq!(raw_to_percent(100.0), 100.0);
        assert_eq!(percent_to_raw(50.0), 0.0);
        assert_eq!(percent_to_raw(0.0), -100.0);
        assert_eq!(percent_to_raw(100.0), 100.0);
    }

    /// `mood.affinityGainRate` aceita 0..10: com 0.2 cada conversa soma 0.4 na escala -100..100, e o
    /// `round()` na gravação devolvia 0 — a afinidade nunca saía do lugar. O progresso fracionário tem
    /// de acumular.
    #[test]
    fn a_small_gain_rate_still_accumulates_instead_of_rounding_to_zero() {
        let (config, _dir) = store();
        config.set("mood.affinityGainRate", json!(0.2)).unwrap();
        let affinity = PersistentAffinity::new(Arc::clone(&config));

        for _ in 0..10 {
            affinity.add_interaction(1.0).unwrap();
        }

        let raw = config.get("character.affinityScore").and_then(|v| v.as_f64()).unwrap();
        assert!((raw - 4.0).abs() < 1e-9, "10 interações de +0.2% deviam somar 4.0 na escala bruta, ficou {raw}");
    }

    #[test]
    fn add_interaction_persists_into_character_affinity_score() {
        let (config, _dir) = store();
        let affinity = PersistentAffinity::new(Arc::clone(&config));

        let (snapshot, _) = affinity.add_interaction(1.0).unwrap();
        assert_eq!(snapshot["score"], json!(51.0));

        // Lido de volta do PRÓPRIO ConfigStore, não de um cache do struct — prova a persistência.
        // 51% na escala 0..100 vira 2 na escala -100..100 de `character.affinityScore`.
        assert_eq!(config.get("character.affinityScore"), Some(json!(2.0)));
    }

    /// NEGATIVO — leitura sempre parte do valor gravado por outro escritor, nunca de um cache local
    /// desatualizado (o motivo de existir `readExternalScore` no TS).
    #[test]
    fn reads_fresh_even_when_another_writer_changed_the_score_between_calls() {
        let (config, _dir) = store();
        let affinity = PersistentAffinity::new(Arc::clone(&config));

        affinity.add_interaction(1.0).unwrap();
        // Um "terceiro escritor" (ex.: a tool avatar/affinity_delta) grava por fora.
        config.set("character.affinityScore", json!(80.0)).unwrap();

        let snapshot = affinity.snapshot();
        assert_eq!(snapshot["score"], json!(90.0), "não leu o valor gravado por outro escritor");
    }

    #[tokio::test]
    async fn handler_get_reports_affinity_as_persisted() {
        let (config, _dir) = store();
        let handler = PersistentMoodToolHandler { affinity: PersistentAffinity::new(config) };

        let result = handler.call(json!({ "action": "get" })).await.unwrap();
        assert_eq!(result["affinityPersisted"], json!(true));
        assert_eq!(result["data"]["affinity"]["score"], json!(50.0));
    }

    #[tokio::test]
    async fn register_overwrites_the_non_persistent_mood_tool() {
        let _order = crate::modules::features::mood::TOOL_REGISTRATION_ORDER
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (config, _dir) = store();
        crate::modules::features::mood::register_tools();
        register(config);

        let result = crate::modules::tools::registry()
            .execute("mood", json!({ "action": "register_interaction", "weight": 2.0 }))
            .await
            .unwrap();
        assert_eq!(result["affinityPersisted"], json!(true), "o registro sem config não foi substituído");
    }
}
