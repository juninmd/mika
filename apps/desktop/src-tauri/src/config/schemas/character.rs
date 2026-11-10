//! Namespaces de configuração do módulo `character`, portados dos `*.schema.ts` correspondentes.
//!
//! Fontes: `character.schema.ts` (aba + grupo de tema), `character-identity-config.schema.ts`,
//! `character-habits-config.schema.ts`, `character-tone-config.schema.ts` e
//! `character-relationship-config.schema.ts`. Todos os grupos gravam sob o mesmo prefixo
//! `ConfigKey.Character` ('character'), por isso viram um único `Namespace`.

use super::super::registry::Namespace;
use super::super::schema::{boolean, choice, float_range, map, text, FieldUi, UiOption};

pub fn namespaces() -> Vec<Namespace> {
    vec![Namespace {
        name: "character",
        fields: vec![
            // --- character-identity-config.schema.ts ---
            boolean("enabled", true).with_ui(
                FieldUi::new("settings.character.field.enabled.label")
                    .described("settings.character.field.enabled.description"),
            ),
            text("name", "Mika").with_ui(FieldUi::new("settings.character.field.name.label")),
            text("personality", "sarcástica, inteligente, nerd, apaixonada por games e anime").with_ui(
                FieldUi::new("settings.character.field.personality.label")
                    .control("textarea")
                    .described("settings.character.field.personality.description"),
            ),
            // Chave na MESMA linha do construtor — o gate de paridade lê campo por regex
            // (`choice\("chave"` / `text\("chave"`) e não enxerga uma chave que abra em linha
            // própria.
            choice("personalityProfile", "friend", &["professional", "friend", "tsundere", "sarcastic"])
                .with_ui(
                FieldUi::new("settings.character.field.personalityProfile.label")
                    .described("settings.character.field.personalityProfile.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.personalityProfile.option.professional",
                            value: "professional",
                        },
                        UiOption {
                            label: "settings.character.field.personalityProfile.option.friend",
                            value: "friend",
                        },
                        UiOption {
                            label: "settings.character.field.personalityProfile.option.tsundere",
                            value: "tsundere",
                        },
                        UiOption {
                            label: "settings.character.field.personalityProfile.option.sarcastic",
                            value: "sarcastic",
                        },
                    ]),
            ),
            text("visualDescription",
                "Anime VTuber com presença confiante e energia geek.",
            )
            .with_ui(
                FieldUi::new("settings.character.field.visualDescription.label")
                    .control("textarea")
                    .helper("settings.character.field.visualDescription.helperText"),
            ),
            text("greetingStyle", "casual, espirituosa, nerd, observadora")
                .with_ui(FieldUi::new("settings.character.field.greetingStyle.label")),
            choice("responseStyle", "playful", &["playful", "balanced", "formal"]).with_ui(
                FieldUi::new("settings.character.field.responseStyle.label").options(&[
                    UiOption {
                        label: "settings.character.field.responseStyle.option.playful",
                        value: "playful",
                    },
                    UiOption {
                        label: "settings.character.field.responseStyle.option.balanced",
                        value: "balanced",
                    },
                    UiOption {
                        label: "settings.character.field.responseStyle.option.formal",
                        value: "formal",
                    },
                ]),
            ),
            text("modelName", "default").with_ui(
                FieldUi::new("settings.character.field.modelName.label")
                    .described("settings.character.field.modelName.description"),
            ),
            // `DEFAULT_MODEL_PATH` do lado TypeScript (`shared/avatar-model.ts`).
            text("modelPath", "vrm/mika/mika.vrm").with_ui(
                FieldUi::new("settings.character.field.modelPath.label")
                    .described("settings.character.field.modelPath.description")
                    .helper("settings.character.field.modelPath.helperText"),
            ),
            choice("backgroundMode",
                "transparent",
                &["transparent", "semi-transparent", "solid"],
            )
            .with_ui(
                FieldUi::new("settings.character.field.backgroundMode.label")
                    .described("settings.character.field.backgroundMode.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.backgroundMode.option.transparent",
                            value: "transparent",
                        },
                        UiOption {
                            label: "settings.character.field.backgroundMode.option.semiTransparent",
                            value: "semi-transparent",
                        },
                        UiOption {
                            label: "settings.character.field.backgroundMode.option.solid",
                            value: "solid",
                        },
                    ]),
            ),
            text("backgroundSolidColor", "#050914").with_ui(
                FieldUi::new("settings.character.field.backgroundSolidColor.label")
                    .helper("settings.character.field.backgroundSolidColor.helperText"),
            ),
            // Vento no avatar: molas do VRM (cabelo, saia, fitas) e cabelo do Live2D. Lido pelo
            // renderer (`useAvatarWind.ts` → `windStrengthFromConfig`), controle na aba Skin.
            boolean("windEnabled", true).with_ui(
                FieldUi::new("settings.character.field.windEnabled.label")
                    .described("settings.character.field.windEnabled.description"),
            ),
            float_range("windStrength", 35.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.windStrength.label")
                    .described("settings.character.field.windStrength.description"),
            ),
            // Nitidez do render do avatar (densidade de pixels dos canvases VRM/Live2D). Lida pelo
            // renderer (`useRenderSharpness.ts` → `render-pixel-ratio.ts`), controle na aba Skin.
            choice("renderSharpness", "auto", &["auto", "performance", "sharp"]).with_ui(
                FieldUi::new("settings.character.field.renderSharpness.label")
                    .described("settings.character.field.renderSharpness.description")
                    .options(&[
                        UiOption { label: "settings.character.field.renderSharpness.option.auto", value: "auto" },
                        UiOption {
                            label: "settings.character.field.renderSharpness.option.performance",
                            value: "performance",
                        },
                        UiOption { label: "settings.character.field.renderSharpness.option.sharp", value: "sharp" },
                    ]),
            ),
            // --- character.schema.ts: grupo "appearance" (tema) ---
            choice("theme",
                "neural-glass",
                &[
                    "neural-glass",
                    "dark",
                    "light",
                    "sakura",
                    "cyberpunk",
                    "neon-mint",
                    "aurora",
                    "synthwave",
                    "midnight-galaxy",
                    "ocean-deep",
                ],
            )
            .with_ui(
                FieldUi::new("settings.character.field.theme.label")
                    .described("settings.character.field.theme.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.theme.option.neuralGlass",
                            value: "neural-glass",
                        },
                        UiOption {
                            label: "settings.character.field.theme.option.dark",
                            value: "dark",
                        },
                        UiOption {
                            label: "settings.character.field.theme.option.light",
                            value: "light",
                        },
                        // Estas opções eram texto literal (não chave i18n) no original — copiadas
                        // como estão, sem inventar chave nova.
                        UiOption { label: "Sakura", value: "sakura" },
                        UiOption { label: "Cyberpunk", value: "cyberpunk" },
                        UiOption { label: "Neon Mint", value: "neon-mint" },
                        UiOption { label: "Aurora", value: "aurora" },
                        UiOption { label: "Synthwave", value: "synthwave" },
                        UiOption {
                            label: "Midnight Galaxy",
                            value: "midnight-galaxy",
                        },
                        UiOption {
                            label: "Ocean Deep",
                            value: "ocean-deep",
                        },
                    ]),
            ),
            // --- character-tone-config.schema.ts ---
            choice("moodBaseline",
                "playful",
                &["calm", "energetic", "playful", "serious", "chaotic"],
            )
            .with_ui(
                FieldUi::new("settings.character.field.moodBaseline.label")
                    .described("settings.character.field.moodBaseline.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.moodBaseline.option.calm",
                            value: "calm",
                        },
                        UiOption {
                            label: "settings.character.field.moodBaseline.option.energetic",
                            value: "energetic",
                        },
                        UiOption {
                            label: "settings.character.field.moodBaseline.option.playful",
                            value: "playful",
                        },
                        UiOption {
                            label: "settings.character.field.moodBaseline.option.serious",
                            value: "serious",
                        },
                        UiOption {
                            label: "settings.character.field.moodBaseline.option.chaotic",
                            value: "chaotic",
                        },
                    ]),
            ),
            float_range("energyLevel", 70.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.energyLevel.label")
                    .described("settings.character.field.energyLevel.description"),
            ),
            choice("formalityLevel", "casual", &["casual", "balanced", "formal"]).with_ui(
                FieldUi::new("settings.character.field.formalityLevel.label")
                    .described("settings.character.field.formalityLevel.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.formalityLevel.option.casual",
                            value: "casual",
                        },
                        UiOption {
                            label: "settings.character.field.formalityLevel.option.balanced",
                            value: "balanced",
                        },
                        UiOption {
                            label: "settings.character.field.formalityLevel.option.formal",
                            value: "formal",
                        },
                    ]),
            ),
            choice("humorStyle",
                "witty",
                &["dry", "witty", "silly", "sarcastic", "wholesome"],
            )
            .with_ui(
                FieldUi::new("settings.character.field.humorStyle.label")
                    .described("settings.character.field.humorStyle.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.humorStyle.option.dry",
                            value: "dry",
                        },
                        UiOption {
                            label: "settings.character.field.humorStyle.option.witty",
                            value: "witty",
                        },
                        UiOption {
                            label: "settings.character.field.humorStyle.option.silly",
                            value: "silly",
                        },
                        UiOption {
                            label: "settings.character.field.humorStyle.option.sarcastic",
                            value: "sarcastic",
                        },
                        UiOption {
                            label: "settings.character.field.humorStyle.option.wholesome",
                            value: "wholesome",
                        },
                    ]),
            ),
            float_range("sarcasmLevel", 60.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.sarcasmLevel.label")
                    .described("settings.character.field.sarcasmLevel.description"),
            ),
            float_range("chaosFactor", 20.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.chaosFactor.label")
                    .described("settings.character.field.chaosFactor.description"),
            ),
            float_range("tsundereIntensity", 30.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.tsundereIntensity.label")
                    .described("settings.character.field.tsundereIntensity.description"),
            ),
            // --- character-habits-config.schema.ts ---
            boolean("slangUsage", true).with_ui(
                FieldUi::new("settings.character.field.slangUsage.label")
                    .described("settings.character.field.slangUsage.description"),
            ),
            boolean("emojiUsage", true).with_ui(
                FieldUi::new("settings.character.field.emojiUsage.label")
                    .described("settings.character.field.emojiUsage.description"),
            ),
            choice("jokeFrequency", "occasional", &["rare", "occasional", "frequent"]).with_ui(
                FieldUi::new("settings.character.field.jokeFrequency.label")
                    .described("settings.character.field.jokeFrequency.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.jokeFrequency.option.rare",
                            value: "rare",
                        },
                        UiOption {
                            label: "settings.character.field.jokeFrequency.option.occasional",
                            value: "occasional",
                        },
                        UiOption {
                            label: "settings.character.field.jokeFrequency.option.frequent",
                            value: "frequent",
                        },
                    ]),
            ),
            float_range("proactiveness", 50.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.proactiveness.label")
                    .described("settings.character.field.proactiveness.description"),
            ),
            float_range("empathyLevel", 70.0, 0.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.empathyLevel.label")
                    .described("settings.character.field.empathyLevel.description"),
            ),
            choice("swearTolerance", "none", &["none", "mild", "moderate"]).with_ui(
                FieldUi::new("settings.character.field.swearTolerance.label")
                    .described("settings.character.field.swearTolerance.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.swearTolerance.option.none",
                            value: "none",
                        },
                        UiOption {
                            label: "settings.character.field.swearTolerance.option.mild",
                            value: "mild",
                        },
                        UiOption {
                            label: "settings.character.field.swearTolerance.option.moderate",
                            value: "moderate",
                        },
                    ]),
            ),
            choice("responseLength", "balanced", &["concise", "balanced", "detailed"]).with_ui(
                FieldUi::new("settings.character.field.responseLength.label")
                    .described("settings.character.field.responseLength.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.responseLength.option.concise",
                            value: "concise",
                        },
                        UiOption {
                            label: "settings.character.field.responseLength.option.balanced",
                            value: "balanced",
                        },
                        UiOption {
                            label: "settings.character.field.responseLength.option.detailed",
                            value: "detailed",
                        },
                    ]),
            ),
            choice("memoryRecall", "selective", &["selective", "eager", "obsessive"]).with_ui(
                FieldUi::new("settings.character.field.memoryRecall.label")
                    .described("settings.character.field.memoryRecall.description")
                    .options(&[
                        UiOption {
                            label: "settings.character.field.memoryRecall.option.selective",
                            value: "selective",
                        },
                        UiOption {
                            label: "settings.character.field.memoryRecall.option.eager",
                            value: "eager",
                        },
                        UiOption {
                            label: "settings.character.field.memoryRecall.option.obsessive",
                            value: "obsessive",
                        },
                    ]),
            ),
            // `slangEnabled`/`slangDictionary` usam `.describe(<string crua>)` no original, sem
            // `m({...})` — não há metadados de UI estruturados (label/description/options) para
            // portar; ambos ficam sem `.with_ui(...)`.
            boolean("slangEnabled", true),
            map("slangDictionary"),
            // `character-habits-config.schema.ts`: `z.number()` sem `.int()` — Float, não Int.
            float_range("quietHoursStart", 23.0, 0.0, 23.0).with_ui(
                FieldUi::new("settings.character.field.quietHoursStart.label")
                    .described("settings.character.field.quietHoursStart.description"),
            ),
            float_range("quietHoursEnd", 7.0, 0.0, 23.0).with_ui(
                FieldUi::new("settings.character.field.quietHoursEnd.label")
                    .described("settings.character.field.quietHoursEnd.description"),
            ),
            // --- character-relationship-config.schema.ts ---
            choice("relationshipStatus",
                "friend",
                &["acquaintance", "friend", "close-friend", "companion"],
            )
            .with_ui(
                FieldUi::new("settings.character.field.relationshipStatus.label")
                    .described("settings.character.field.relationshipStatus.description"),
            ),
            float_range("affinityScore", 0.0, -100.0, 100.0).with_ui(
                FieldUi::new("settings.character.field.affinityScore.label")
                    .described("settings.character.field.affinityScore.description"),
            ),
            text("hobbies",
                "programação, games (RPG, FPS, roguelikes), anime (shonen, seinen, isekai), hardware, tech news",
            )
            .with_ui(
                FieldUi::new("settings.character.field.hobbies.label")
                    .described("settings.character.field.hobbies.description"),
            ),
            text("dislikes", "grosseria, internet lenta, bugs idiotas, spoilers sem aviso, lag").with_ui(
                FieldUi::new("settings.character.field.dislikes.label")
                    .described("settings.character.field.dislikes.description"),
            ),
            text("nicknamesForUser", "Usuário, Senpai, Mestre").with_ui(
                FieldUi::new("settings.character.field.nicknamesForUser.label")
                    .described("settings.character.field.nicknamesForUser.description"),
            ),
            text("secretGoals", "Construir o melhor relacionamento humano-IA").with_ui(
                FieldUi::new("settings.character.field.secretGoals.label")
                    .described("settings.character.field.secretGoals.description"),
            ),
            text("customInstructions", "").with_ui(
                FieldUi::new("settings.character.field.customInstructions.label")
                    .control("textarea")
                    .described("settings.character.field.customInstructions.description"),
            ),
        ],
    }]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{FieldKind, FieldSchema};

    fn field(key: &str) -> FieldSchema {
        namespaces()[0].fields.iter().find(|field| field.key == key).cloned().expect("campo declarado")
    }

    /// The theme picker offers the renderer's registry; an id the schema refuses is a click that
    /// silently never persists.
    #[test]
    fn every_renderer_theme_is_a_valid_theme_and_the_default_exists() {
        let renderer = [
            include_str!("../../../../src/renderer/src/themes.ts"),
            include_str!("../../../../src/renderer/src/themes-extra.ts"),
        ];
        let id_re = regex::Regex::new(r"(?m)^\s*id: '([a-z-]+)',").unwrap();
        let ids: Vec<String> = renderer.iter().flat_map(|src| id_re.captures_iter(src).map(|c| c[1].to_string())).collect();
        assert!(ids.len() >= 5, "extração de temas quebrou: {ids:?}");
        let theme = field("theme");
        let FieldKind::Enum { variants } = theme.kind else { panic!("theme deveria ser enum") };
        for id in &ids {
            assert!(variants.contains(&id.as_str()), "THEMES tem '{id}' e o schema recusa");
        }
        assert!(ids.iter().any(|id| theme.default == *id), "default {} não existe no renderer", theme.default);
    }

    /// The renderer applies `DEFAULT_WIND_STRENGTH` until the config arrives: the schema default must
    /// be the same breeze, or the first seconds after boot blow harder (or softer) than the setting.
    #[test]
    fn wind_defaults_match_the_renderer_and_stay_in_range() {
        let renderer = include_str!("../../../../src/renderer/src/components/avatar/vrm-wind.ts");
        let strength = field("windStrength");
        let default = strength.default.as_f64().expect("windStrength numérico");
        let expected = format!("export const DEFAULT_WIND_STRENGTH = {}", default / 100.0);
        assert!(renderer.contains(&expected), "windStrength default {default} diverge de DEFAULT_WIND_STRENGTH");
        let FieldKind::Float { min, max } = strength.kind else { panic!("windStrength deveria ser número") };
        assert_eq!((min, max), (Some(0.0), Some(100.0)));
        assert_eq!(field("windEnabled").default, serde_json::json!(true));
    }

    /// The Skin tab offers the renderer's sharpness modes: one the schema refuses is a click that
    /// never persists, and one only the schema knows silently renders as `auto`.
    #[test]
    fn render_sharpness_modes_and_default_match_the_renderer() {
        let renderer = include_str!("../../../../src/renderer/src/lib/render-pixel-ratio.ts");
        let sharpness = field("renderSharpness");
        let default = sharpness.default.as_str().expect("renderSharpness textual").to_string();
        let FieldKind::Enum { variants } = sharpness.kind else { panic!("renderSharpness deveria ser enum") };
        let modes = variants.iter().map(|mode| format!("'{mode}'")).collect::<Vec<_>>().join(", ");
        let expected_modes = format!("export const RENDER_SHARPNESS_MODES = [{modes}] as const");
        assert!(renderer.contains(&expected_modes), "modos do schema divergem do renderer: {modes}");
        let expected_default = format!("export const DEFAULT_RENDER_SHARPNESS: RenderSharpness = '{default}'");
        assert!(renderer.contains(&expected_default), "default '{default}' diverge de DEFAULT_RENDER_SHARPNESS");
    }

    /// Renderer and schema once disagreed on the default avatar (a third-party sample not shipped).
    #[test]
    fn the_default_avatar_is_the_one_the_renderer_ships() {
        let renderer = include_str!("../../../../src/shared/avatar-model.ts");
        let default = field("modelPath").default;
        let expected = format!("export const DEFAULT_MODEL_PATH = '{}'", default.as_str().unwrap());
        assert!(renderer.contains(&expected), "modelPath default {default} diverge de DEFAULT_MODEL_PATH");
    }
}
