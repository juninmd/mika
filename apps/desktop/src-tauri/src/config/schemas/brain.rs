//! Namespaces de configuração do módulo `brain`, portados dos `*.schema.ts` correspondentes.
//!
//! Fonte: `apps/desktop/src/main/services/brain/brain.schema.ts`.
//!
//! O schema TypeScript declara DOIS grupos: um com `prefix: ConfigKey.Brain` (os campos abaixo) e
//! um segundo, `memoryConfig`, com `prefix: ConfigKey.Memory` — mas `ConfigKey.Memory` é o MESMO
//! namespace `memory` já portado em `memory.schema.ts` (`schemas/mod.rs::memory()`). É a mesma tela
//! de Settings reexibindo os campos de `memory` dentro da aba Brain; não é um schema novo. Portar
//! esse segundo grupo aqui duplicaria o namespace `memory` e `SchemaRegistry::new` derruba o boot
//! nesse caso (`registro de config 'memory' declarado duas vezes`) — por isso só o grupo `brain` é
//! portado neste arquivo.

use super::super::registry::Namespace;
use super::super::schema::{boolean, float_range, int_range, list, FieldUi};

/// `brain.schema.ts` (grupo `prefix: ConfigKey.Brain`).
///
/// **`autonomousThoughtsEnabled`/`autonomousThoughtsIntervalMin` removidos nesta rodada.** O
/// provider de "pensamento autônomo" periódico do TS nunca foi portado — `modules/brain/thought.rs`
/// documenta explicitamente que só `handleProactiveEvent` foi trazido, e nomeia o resto como
/// pendente. Sem `BrainStateMachine`/ciclo de intervalo nenhum consumindo estas duas chaves, os
/// controles em Settings prometiam uma cadência que nunca rodava.
fn brain() -> Namespace {
    Namespace {
        name: "brain",
        fields: vec![
            // `.describe('settings.brain.field.enabled')` é string pura, não `m({...})`: o
            // parser de metadados (`zod-inference.ts`) usa a MESMA string como label e description.
            boolean("enabled", true)
                .with_ui(FieldUi::new("settings.brain.field.enabled").described("settings.brain.field.enabled")),
            boolean("embeddingEnabled", true).with_ui(
                FieldUi::new("settings.brain.field.embeddingEnabled")
                    .described("settings.brain.field.embeddingEnabled"),
            ),
            float_range("recencyBias", 0.25, 0.0, 1.0)
                .with_ui(FieldUi::new("settings.brain.field.recencyBias.label")),
            boolean("rerankingEnabled", true).with_ui(
                FieldUi::new("settings.brain.field.rerankingEnabled")
                    .described("settings.brain.field.rerankingEnabled"),
            ),
            boolean("emotionalTagging", true).with_ui(
                FieldUi::new("settings.brain.field.emotionalTagging")
                    .described("settings.brain.field.emotionalTagging"),
            ),
            int_range("ragTopK", 5, 1, 20).with_ui(FieldUi::new("settings.brain.field.ragTopK.label")),
            float_range("minSimilarity", 0.25, 0.0, 1.0)
                .with_ui(FieldUi::new("settings.brain.field.minSimilarity.label")),
            float_range("lexicalRerankWeight", 0.15, 0.0, 0.5)
                .with_ui(FieldUi::new("settings.brain.field.lexicalRerankWeight.label")),
            boolean("approveRiskyTools", true).with_ui(
                FieldUi::new("settings.brain.field.approveRiskyTools")
                    .described("settings.brain.field.approveRiskyTools"),
            ),
            list("autoApproveTools").with_ui(
                FieldUi::new("settings.brain.field.autoApproveTools")
                    .described("settings.brain.field.autoApproveTools"),
            ),
        ],
    }
}

pub fn namespaces() -> Vec<Namespace> {
    vec![brain()]
}
