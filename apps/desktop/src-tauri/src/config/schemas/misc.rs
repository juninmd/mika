//! Namespaces de configuração do módulo `misc`, portados dos `*.schema.ts` correspondentes.
//!
//! Reúne os schemas menores que não pertencem a `brain`, `character`, `gamer`, `llm`, `user`,
//! `vision` nem `voice`: `privacy`, `mcp`, `person-catalog`, `browser`, `mood` e `scheduler`.

use super::super::registry::Namespace;
use super::super::schema::{boolean, choice, float_range, int_range, text, FieldUi, UiOption};

/// `apps/desktop/src/features/privacy/privacy.schema.ts`
///
/// As duas `groups` do lado TypeScript compartilham o mesmo `prefix` (`ConfigKey.Privacy`), então
/// viram um único namespace aqui — o store não distingue grupo de UI, só namespace.
fn privacy() -> Namespace {
    Namespace {
        name: "privacy",
        fields: vec![
            // Chave na MESMA linha do `choice(` — o gate de paridade lê campo por regex
            // (`choice\("chave"`) e não enxerga um argumento que abra em linha própria.
            choice("conversationRetention", "forever", &["1day", "7days", "30days", "90days", "forever"])
                .with_ui(FieldUi::new("settings.privacy.field.conversationRetention.label")),
            boolean("screenCapture", false)
                .with_ui(FieldUi::new("settings.privacy.field.screenCapture.label")),
            boolean("clipboardLogging", false)
                .with_ui(FieldUi::new("settings.privacy.field.clipboardLogging.label")),
            boolean("offlineMode", false).with_ui(
                FieldUi::new("settings.privacy.field.offlineMode.label")
                    .described("settings.privacy.field.offlineMode.description"),
            ),
        ],
    }
}

/// `apps/desktop/src/features/mcp/mcp.schema.ts`
fn mcp() -> Namespace {
    Namespace {
        name: "mcp",
        fields: vec![
            boolean("enabled", false).with_ui(
                FieldUi::new("settings.mcp.field.enabled.label")
                    .described("settings.mcp.field.enabled.description"),
            ),
            int_range("timeoutSeconds", 20, 2, 120).with_ui(
                FieldUi::new("settings.mcp.field.timeoutSeconds.label")
                    .described("settings.mcp.field.timeoutSeconds.description"),
            ),
        ],
    }
}

/// `apps/desktop/src/features/person-catalog/person-catalog.schema.ts`
///
/// `enabled` e `autoExtractOnChat` usam `.describe(string)` puro no TypeScript, não
/// `m({...})`/`JSON.stringify({...})`. `parseDescribeMeta` (zod-inference.ts) trata uma string que
/// não começa com `{` como label E description ao mesmo tempo — não é omissão do port, é o mesmo
/// valor nos dois campos.
fn person_catalog() -> Namespace {
    Namespace {
        name: "person-catalog",
        fields: vec![
            boolean("enabled", true).with_ui(
                FieldUi::new("settings.personCatalog.field.enabled")
                    .described("settings.personCatalog.field.enabled"),
            ),
            boolean("autoExtractOnChat", true).with_ui(
                FieldUi::new("settings.personCatalog.field.autoExtractOnChat")
                    .described("settings.personCatalog.field.autoExtractOnChat"),
            ),
            // Opt-in: manda o NOME da pessoa recém-catalogada à Wikipédia e aos buscadores
            // (`enrichment.rs`). Desligado, o catálogo continua 100% local.
            boolean("webEnrichment", false).with_ui(
                FieldUi::new("settings.personCatalog.field.webEnrichment.label")
                    .described("settings.personCatalog.field.webEnrichment.description"),
            ),
            // Aviso único do painel: a migração v2→v3 liga isto num perfil que já existia sem escolha
            // gravada de `webEnrichment` (o default antigo era ligado; o novo é desligado). O painel
            // zera ao dispensar.
            boolean("webEnrichmentNoticePending", false),
            int_range("maxMentionsPerPerson", 100, 1, 500).with_ui(
                FieldUi::new("settings.personCatalog.field.maxMentionsPerPerson.label")
                    .helper("settings.personCatalog.field.maxMentionsPerPerson.helperText"),
            ),
        ],
    }
}

/// `apps/desktop/src/main/services/browser/browser.schema.ts`
fn browser() -> Namespace {
    Namespace {
        name: "browser",
        fields: vec![
            // Opt-in (PRIVACY.md §3): busca web e leitura de páginas mandam a consulta e o IP a
            // terceiros. Perfil novo nasce desligado; `config.json` já gravado mantém o valor.
            boolean("enabled", false).with_ui(
                FieldUi::new("settings.browser.field.enabled.label")
                    .described("settings.browser.field.enabled.description"),
            ),
            choice("searchEngine", "auto", &["auto", "bing", "duckduckgo", "yahoo"]).with_ui(
                FieldUi::new("settings.browser.field.searchEngine.label").options(&[
                    UiOption {
                        label: "settings.browser.field.searchEngine.option.auto",
                        value: "auto",
                    },
                    UiOption {
                        label: "settings.browser.field.searchEngine.option.bing",
                        value: "bing",
                    },
                    UiOption {
                        label: "settings.browser.field.searchEngine.option.duckduckgo",
                        value: "duckduckgo",
                    },
                    UiOption {
                        label: "settings.browser.field.searchEngine.option.yahoo",
                        value: "yahoo",
                    },
                ]),
            ),
        ],
    }
}

/// `packages/mood/src/mood.schema.ts`
///
/// As três taxas são `z.number()` SEM `.int()` — ficam `Float`, não `Int`, mesmo com defaults
/// inteiros (`affinityGainRate` default `1`).
fn mood() -> Namespace {
    Namespace {
        name: "mood",
        fields: vec![
            boolean("enabled", true).with_ui(
                FieldUi::new("settings.mood.field.enabled").described("settings.mood.field.enabled"),
            ),
            float_range("energyDecayRate", 0.02, 0.0, 1.0).with_ui(
                FieldUi::new("settings.mood.field.energyDecayRate.label")
                    .helper("settings.mood.field.energyDecayRate.helperText"),
            ),
            float_range("affinityGainRate", 1.0, 0.0, 10.0).with_ui(
                FieldUi::new("settings.mood.field.affinityGainRate.label")
                    .helper("settings.mood.field.affinityGainRate.helperText"),
            ),
            float_range("minAffinityInteractionTime", 30.0, 0.0, 3600.0).with_ui(
                FieldUi::new("settings.mood.field.minAffinityInteractionTime.label")
                    .helper("settings.mood.field.minAffinityInteractionTime.helperText"),
            ),
            // Corpo simulado (`features::mood::needs`) e hábitos de uso
            // (`features::desktop::usage`) — seção "Rotina" do `MoodPanel`.
            boolean("needsEnabled", true).with_ui(
                FieldUi::new("settings.mood.field.needsEnabled.label")
                    .described("settings.mood.field.needsEnabled.description"),
            ),
            float_range("needsIntensity", 1.0, 0.0, 2.0).with_ui(
                FieldUi::new("settings.mood.field.needsIntensity.label")
                    .helper("settings.mood.field.needsIntensity.helperText"),
            ),
            boolean("habitsEnabled", true).with_ui(
                FieldUi::new("settings.mood.field.habitsEnabled.label")
                    .described("settings.mood.field.habitsEnabled.description"),
            ),
        ],
    }
}

/// `packages/scheduler/src/scheduler.schema.ts`
///
/// `scheduler-config.schema.ts` (mesmo pacote) não declara campo algum: é só
/// `sliceSchema(SchedulerSchema, 'scheduler')`, um recorte do schema acima para outro consumidor.
/// Não gera namespace próprio.
///
/// Campo em linha própria, com `],` isolado no fechamento: um `vec![boolean(...)]` cujo `]` final
/// ficasse colado no `)` escapa do regex do gate de paridade (que exige o `]` precedido de newline,
/// e `name: "...",` colado em `fields: vec![` sem comentário no meio) e faz a busca vazar para o
/// namespace seguinte — foi assim que `memory.backend`, `redisUrl`, `shortTermTtlHours`,
/// `longTermTtlDays` e `episodicListLimit` (de `mod.rs`) apareceram, errado, como campos deste
/// namespace.
fn scheduler() -> Namespace {
    Namespace {
        name: "scheduler",
        fields: vec![
            boolean("enabled", true).with_ui(
                FieldUi::new("settings.scheduler.field.enabled")
                    .described("settings.scheduler.field.enabled"),
            ),
        ],
    }
}

/// `system.logLevel`: the Logs panel's level (`bootstrap/tools.ts` had it in its own Zod object);
/// applied by `log_level.rs`.
fn system() -> Namespace {
    Namespace {
        name: "system",
        fields: vec![
            choice("logLevel", "info", &["debug", "info", "warn", "error"]),
        ],
    }
}

/// Device picks from `useAudioDevices`; empty means the OS default device.
fn audio() -> Namespace {
    Namespace {
        name: "audio",
        fields: vec![text("inputDeviceId", ""), text("outputDeviceId", "")],
    }
}

/// `computer-use.enabled` — master switch for the `computer` tool (WP5, native input simulation:
/// click/type/key). Default `false` (opt-in, PRIVACY.md §3): a new profile does not get a tool that
/// can drive the mouse and keyboard until the user turns it on in the Computer control tab. Even
/// then every call goes through the approval broker (`register_destructive`, see
/// `modules/features/computer_use/tool.rs`) — the switch decides whether the tool exists at all,
/// approval decides whether any single call runs. An existing `config.json` keeps its stored value.
fn computer_use() -> Namespace {
    Namespace {
        name: "computer-use",
        fields: vec![boolean("enabled", false).with_ui(
            FieldUi::new("settings.computerUse.field.enabled.label")
                .described("settings.computerUse.field.enabled.description"),
        )],
    }
}

pub fn namespaces() -> Vec<Namespace> {
    vec![
        privacy(),
        mcp(),
        person_catalog(),
        browser(),
        mood(),
        scheduler(),
        system(),
        audio(),
        computer_use(),
    ]
}
