//! Namespaces de configuração do módulo `vision`, portados dos `*.schema.ts` correspondentes.
//!
//! `apps/desktop/src/main/services/vision/vision.schema.ts`

use super::super::registry::Namespace;
use super::super::schema::{boolean, choice, float_range, int_range, text, FieldUi, UiOption};

pub fn namespaces() -> Vec<Namespace> {
    vec![vision()]
}

/// `VisionSettingsGroupSchema` mesclado com `VisionAwarenessGroupSchema` — os dois grupos TS
/// caem sob o mesmo prefixo `vision` (`VisionConfigSchema = ...merge(...)`), então viram um único
/// namespace aqui.
fn vision() -> Namespace {
    Namespace {
        name: "vision",
        fields: vec![
            boolean("enabled", false).with_ui(FieldUi::new("settings.vision.field.enabled")),
            // Variantes = `VISION_PROVIDER_IDS = ['default', ...LLM_PROVIDER_IDS]`, e
            // `LLM_PROVIDER_IDS = ['google', 'openrouter', 'ollama', 'openaiCompatible', 'unsloth']`
            // (`packages/sdk/src/llm/llm-catalog-data.ts`) — ordem de validação, não a de exibição
            // no dropdown (essa segue a ordem do `options` abaixo, que é outra).
            // Chave na MESMA linha do `choice(` — o gate de paridade lê campo por regex e não
            // enxerga um argumento que abra em linha própria.
            choice("provider", "default", &["default", "google", "openrouter", "ollama", "openaiCompatible", "unsloth"])
                .with_ui(
                    FieldUi::new("settings.vision.field.provider.label").options(&[
                        UiOption {
                            label: "settings.vision.field.provider.option.default",
                            value: "default",
                        },
                        UiOption {
                            label: "settings.vision.field.provider.option.openrouter",
                            value: "openrouter",
                        },
                        UiOption {
                            label: "settings.vision.field.provider.option.google",
                            value: "google",
                        },
                        UiOption {
                            label: "settings.vision.field.provider.option.ollama",
                            value: "ollama",
                        },
                        UiOption {
                            label: "settings.vision.field.provider.option.openaiCompatible",
                            value: "openaiCompatible",
                        },
                        UiOption {
                            label: "settings.vision.field.provider.option.unsloth",
                            value: "unsloth",
                        },
                    ]),
                ),
            text("model", "").with_ui(
                FieldUi::new("settings.vision.field.model.label")
                    .described("settings.vision.field.model.description"),
            ),
            float_range("captureQuality", 80.0, 1.0, 100.0)
                .with_ui(FieldUi::new("settings.vision.field.captureQuality.label")),
            int_range("captureIntervalMs", 60000, 60000, 600000)
                .with_ui(FieldUi::new("settings.vision.field.captureIntervalMs.label")),
            choice("analysisMode", "focused", &["focused", "balanced", "full"]).with_ui(
                FieldUi::new("settings.vision.field.analysisMode.label").options(&[
                    UiOption {
                        label: "settings.vision.field.analysisMode.option.focused",
                        value: "focused",
                    },
                    UiOption {
                        label: "settings.vision.field.analysisMode.option.balanced",
                        value: "balanced",
                    },
                    UiOption {
                        label: "settings.vision.field.analysisMode.option.full",
                        value: "full",
                    },
                ]),
            ),
            boolean("autoDescribeScreen", true)
                .with_ui(FieldUi::new("settings.vision.field.autoDescribeScreen")),
            boolean("adaptiveCompression", true).with_ui(
                FieldUi::new("settings.vision.field.adaptiveCompression.label")
                    .described("settings.vision.field.adaptiveCompression.description"),
            ),
            int_range("maxImageSizeKB", 500, 50, 2000).with_ui(
                FieldUi::new("settings.vision.field.maxImageSizeKB.label")
                    .described("settings.vision.field.maxImageSizeKB.description"),
            ),
            int_range("compressionTargetSizeKB", 300, 50, 1500).with_ui(
                FieldUi::new("settings.vision.field.compressionTargetSizeKB.label")
                    .described("settings.vision.field.compressionTargetSizeKB.description"),
            ),
            int_range("llmQuality", 60, 1, 100)
                .with_ui(FieldUi::new("settings.vision.field.llmQuality")),
            int_range("maxImageDimension", 1280, 320, 4096)
                .with_ui(FieldUi::new("settings.vision.field.maxImageDimension")),
            int_range("maxOutputTokens", 512, 128, 4096)
                .with_ui(FieldUi::new("settings.vision.panel.maxOutputTokensLabel")),
            // Default = `DEFAULT_CAPTURE_BLOCKLIST` de `infrastructure/os/capture-blocklist.ts`.
            // Chave na MESMA linha do `text(` — o gate de paridade lê campo por regex
            // (`text\("chave"`) e não enxerga uma chave que abra em linha própria.
            text("captureBlocklist",
                "bitwarden, 1password, keepass, lastpass, dashlane, proton pass, keeper password, keepersecurity, nordpass, authy, authenticator, 2fa, banco, online banking, mobile banking, internet banking, bank of america, wells fargo, itau, nubank, bradesco, santander, banco inter, caixa, gmail, outlook, thunderbird, proton mail, yahoo mail, whatsapp, telegram, proc:signal, messenger.com, facebook messenger, discord, skype, proc:slack, slack.com, incognito, inprivate, private browsing, navegação privada, janela anônima, aba anônima, metamask, ledger live, ledger wallet, trezor, exodus wallet, proc:exodus, trust wallet, coinbase, binance, mychart, doctolib, teladoc, portal do paciente, prontuario",
            )
            .with_ui(
                FieldUi::new("settings.vision.field.captureBlocklist.label")
                    .control("textarea")
                    .described("settings.vision.field.captureBlocklist.description"),
            ),
            boolean("visionAssistActive", true)
                .with_ui(FieldUi::new("settings.vision.field.visionAssistActive")),
            boolean("visionAssistAutoComment", true)
                .with_ui(FieldUi::new("settings.vision.field.visionAssistAutoComment")),
            boolean("codeAwareness", true).with_ui(FieldUi::new("settings.vision.field.codeAwareness")),
            boolean("gameAwareness", true).with_ui(FieldUi::new("settings.vision.field.gameAwareness")),
            float_range("proactiveCommentaryChance", 0.35, 0.0, 1.0).with_ui(
                FieldUi::new("settings.vision.field.proactiveCommentaryChance.label")
                    .described("settings.vision.field.proactiveCommentaryChance.description"),
            ),
            float_range("textDensityThreshold", 0.12, 0.0, 1.0).with_ui(
                FieldUi::new("settings.vision.field.textDensityThreshold.label")
                    .described("settings.vision.field.textDensityThreshold.description"),
            ),
            boolean("readClipboardImages", false)
                .with_ui(FieldUi::new("settings.vision.field.readClipboardImages")),
            boolean("visualDiaryEnabled", false)
                .with_ui(FieldUi::new("settings.vision.field.visualDiaryEnabled")),
            // `z.number().int().positive()` sem `.max()` no original — sem teto declarado do lado
            // TS, mas o `describe()` documenta 1..100; seguindo a documentação visível, não o
            // Zod puro (que só recusaria <= 0).
            int_range("visualDiaryIntervalCycles", 10, 1, 100).with_ui(
                FieldUi::new("settings.vision.field.visualDiaryIntervalCycles.label")
                    .described("settings.vision.field.visualDiaryIntervalCycles.description"),
            ),
        ],
    }
}
