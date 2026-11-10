//! Namespaces de configuração do módulo `gamer-companion`, portados dos `*.schema.ts`
//! correspondentes.
//!
//! `apps/desktop/src/features/gamer-companion/gamer-companion.schema.ts` +
//! `apps/desktop/src/features/gamer-companion/gamer-companion-config.schema.ts` (esta só faz
//! `sliceSchema(GamerCompanionSchema, 'gamer-companion')`, sem campo próprio).

use serde_json::{json, Value};

use super::super::registry::Namespace;
use super::super::schema::{boolean, float_range, int_range, map, text, FieldSchema, FieldUi};

/// Sobrescreve o default de um `FieldSchema` já construído — usado para campos com default
/// estruturado que os construtores curtos (`boolean`, `text`, `map`...) não expressam sozinhos.
/// A chamada ao construtor (ex.: `map("momentCooldowns")`) precisa continuar literal no ponto de
/// uso: `check:config-schema-parity` lê o campo por regex direto do texto do arquivo.
fn with_default(mut field: FieldSchema, default: Value) -> FieldSchema {
    field.default = default;
    field
}

/// O prefixo TS é `'gamer-companion' as never` (`GAMER_COMPANION_PREFIX`) — com hífen, confirmado
/// em `gamer-companion.module.ts` (`config: ['gamer-companion.']`).
pub fn namespaces() -> Vec<Namespace> {
    vec![gamer_companion()]
}

fn gamer_companion() -> Namespace {
    Namespace {
        name: "gamer-companion",
        fields: vec![
            boolean("enabled", true)
                .with_ui(FieldUi::new("settings.gamerCompanion.field.enabled.label")),
            // Chave na MESMA linha do `text(` — o gate de paridade lê campo por regex
            // (`text\("chave"`) e não enxerga uma chave que abra em linha própria.
            text("favoriteGames",
                "The Witcher 3, Dark Souls, Hades, Hollow Knight, Elden Ring",
            )
            .with_ui(
                FieldUi::new("settings.gamerCompanion.field.favoriteGames.label")
                    .control("textarea")
                    .described("settings.gamerCompanion.field.favoriteGames.description"),
            ),
            text("favoriteAnime",
                "Fullmetal Alchemist: Brotherhood, Vinland Saga, Frieren, Chainsaw Man, Spy x Family",
            )
            .with_ui(
                FieldUi::new("settings.gamerCompanion.field.favoriteAnime.label")
                    .control("textarea")
                    .described("settings.gamerCompanion.field.favoriteAnime.description"),
            ),
            boolean("gameWatcherEnabled", true).with_ui(
                FieldUi::new("settings.gamerCompanion.field.gameWatcherEnabled.label")
                    .described("settings.gamerCompanion.field.gameWatcherEnabled.description"),
            ),
            int_range("gameWatcherIntervalSeconds", 30, 10, 300)
                .with_ui(FieldUi::new("settings.gamerCompanion.field.gameWatcherIntervalSeconds.label")),
            boolean("backseatGamingEnabled", false).with_ui(
                FieldUi::new("settings.gamerCompanion.field.backseatGamingEnabled.label")
                    .described("settings.gamerCompanion.field.backseatGamingEnabled.description"),
            ),
            // `z.number()` sem `.int()` no original — Float, não Int.
            float_range("backseatAggressiveness", 40.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.gamerCompanion.field.backseatAggressiveness.label")
                    .described("settings.gamerCompanion.field.backseatAggressiveness.description"),
            ),
            // `momentCooldowns` é `z.object({...}).default(() => MomentCooldownsSchema.parse({}))`
            // — objeto de forma fixa, não um `z.record(...)`. O modelo de schema aqui não tem um
            // `FieldKind` para objeto de forma fixa (só `Map` livre), então o campo vira `Map` com
            // o default REAL (os 6 defaults por momento), não `{}`.
            with_default(
                map("momentCooldowns").with_ui(
                    FieldUi::new("settings.gamerCompanion.field.momentCooldowns.label")
                        .described("settings.gamerCompanion.field.momentCooldowns.description"),
                ),
                json!({
                    "boss": 90,
                    "achievement": 60,
                    "victory": 60,
                    "death": 45,
                    "cutscene": 45,
                    "defeat": 45,
                }),
            ),
            boolean("sessionSummaryEnabled", true).with_ui(
                FieldUi::new("settings.gamerCompanion.field.sessionSummaryEnabled.label")
                    .described("settings.gamerCompanion.field.sessionSummaryEnabled.description"),
            ),
            // "Jogos personalizados": `{ "processo.exe": "Título" }`, lido por
            // `gamer::game_detection::custom_games` a cada passada do watcher.
            map("customGames").with_ui(
                FieldUi::new("settings.gamerCompanion.field.customGames.label")
                    .described("settings.gamerCompanion.field.customGames.description"),
            ),
            text("steamPath", "").with_ui(
                FieldUi::new("settings.gamerCompanion.field.steamPath.label")
                    .described("settings.gamerCompanion.field.steamPath.description"),
            ),
            // Credencial: default é string vazia, nenhuma chave real entra aqui.
            text("steamApiKey", "").with_ui(
                FieldUi::new("settings.gamerCompanion.field.steamApiKey.label")
                    .control("password")
                    .described("settings.gamerCompanion.field.steamApiKey.description"),
            ),
            text("steamId64", "").with_ui(
                FieldUi::new("settings.gamerCompanion.field.steamId64.label")
                    .described("settings.gamerCompanion.field.steamId64.description"),
            ),
            text("topPlayedCache", "").with_ui(
                FieldUi::new("settings.gamerCompanion.field.topPlayedCache.label")
                    .described("settings.gamerCompanion.field.topPlayedCache.description")
                    .helper("settings.gamerCompanion.field.topPlayedCache.helperText"),
            ),
        ],
    }
}
