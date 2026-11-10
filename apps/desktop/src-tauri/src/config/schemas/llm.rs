//! Namespaces de configuração do módulo `llm`, portados dos `*.schema.ts` correspondentes.
//!
//! Fontes: `llm.schema.ts`, `llm-config.schema.ts`, `llm-model-config.schema.ts`,
//! `llm-behavior-config.schema.ts`, `llm-api-keys-config.schema.ts`, `embedding.schema.ts`.
//!
//! `embedding.schema.ts` declara seu grupo com `prefix: ConfigKey.LLM` (= `'llm'`, ver
//! `ConfigKey.LlmEmbeddingProvider = 'llm.embeddingProvider'` em `config-keys.enum.ts`) — os campos
//! de embedding vivem no MESMO namespace `llm`, não num namespace `embedding` à parte.
//!
//! Só existe aqui campo que o runtime lê: `enableRouting`/`routingTable`/`smartRouter` (roteador
//! de modelos nunca portado) e `maxCostUsdPerTurn` (sem contabilidade de custo por turno) saíram em
//! 2026-09-27 — valores já salvos continuam no `config.json`, o merge sobre defaults os preserva.

use super::llm_defaults;
use super::super::registry::Namespace;
use super::super::schema::{boolean, choice, float_range, int_range, text, FieldUi, UiOption};

/// Rótulos e ids dos cinco provedores de LLM (`packages/sdk/src/llm/llm-catalog-data.ts`). Os
/// rótulos são texto literal do catálogo TypeScript, não chave de i18n — copiados como estão.
const PROVIDER_OPTIONS: &[UiOption] = &[
    UiOption {
        label: "Google AI (Gemini)",
        value: "google",
    },
    UiOption {
        label: "OpenRouter",
        value: "openrouter",
    },
    UiOption {
        label: "Ollama (local)",
        value: "ollama",
    },
    UiOption {
        label: "OpenAI Compatible",
        value: "openaiCompatible",
    },
    UiOption {
        label: "Unsloth (local)",
        value: "unsloth",
    },
];

const PROVIDER_IDS: &[&str] = &["google", "openrouter", "ollama", "openaiCompatible", "unsloth"];
/// Embeddings add `local` (in-process e5, `inference::embedder`): semantic memory with no server.
const EMBEDDING_PROVIDER_IDS: &[&str] = &["local", "google", "openrouter", "ollama", "openaiCompatible", "unsloth"];

/// `llm.schema.ts` (grupo `connection`) + `llm-model-config.schema.ts` + `llm-behavior-config.schema.ts`
/// + `embedding.schema.ts` (mesmo prefixo `llm`).
fn llm() -> Namespace {
    Namespace {
        name: "llm",
        fields: vec![
            choice("provider", "ollama", PROVIDER_IDS).with_ui(
                FieldUi::new("settings.llm.field.provider.label")
                    .control("select")
                    .helper("settings.llm.field.provider.helperText")
                    .options(PROVIDER_OPTIONS),
            ),
            // Defaults = `defaultModel` de `packages/sdk/src/llm/llm-catalog-data.ts` (fontes de cada id
            // lá, revisadas em 2026-09-28).
            // helperText aqui é o texto literal de GOOGLE_PROVIDER.description (catálogo), não
            // uma chave de i18n — copiado como está, sem traduzir nem inventar.
            text("googleModel", llm_defaults::GOOGLE_MODEL).with_ui(
                FieldUi::new("settings.llm.field.googleModel.label")
                    .control("text")
                    .helper("Modelos Gemini via AI SDK com foco em multimodal e baixa latencia."),
            ),
            text("openrouterModel", llm_defaults::OPENROUTER_MODEL).with_ui(
                FieldUi::new("settings.llm.field.openrouterModel.label")
                    .control("text")
                    .helper("Catalogo multi-provider via OpenRouter, seguindo o fluxo do vtuber-ai."),
            ),
            // O campo genérico edita `llm.model` (o `configPath` que `llm-model-config.schema.ts`
            // dava ao campo); o wizard e o painel de conexão gravam `llm.ollamaModel`, que tem
            // precedência (`readModelForProvider`: ollamaModel || model || default). Default vazio
            // para o fallback continuar valendo.
            text("model", llm_defaults::OLLAMA_MODEL).with_ui(
                FieldUi::new("settings.llm.field.ollamaModel.label")
                    .control("text")
                    .helper("Modelos locais servidos por Ollama para uso offline ou privado."),
            ),
            text("ollamaModel", ""),
            text("openaiCompatibleModel", llm_defaults::OPENAI_COMPATIBLE_MODEL).with_ui(
                FieldUi::new("settings.llm.field.openaiCompatibleModel.label")
                    .control("text")
                    .helper(
                        "Qualquer endpoint que fale o protocolo da OpenAI: LiteLLM, vLLM, Ollama (/v1), a própria \
                         OpenAI, NVIDIA NIM.",
                    ),
            ),
            // Default = `defaultModel` do `unsloth` em `llm-catalog-data.ts`: vazio, porque o id do
            // modelo é o que o Unsloth tem carregado (`GET /v1/models`), não um nome fixo.
            text("unslothModel", "").with_ui(
                FieldUi::new("settings.llm.field.unslothModel.label")
                    .control("text")
                    .helper("settings.llm.field.unslothModel.helperText"),
            ),
            float_range("temperature", 0.7, 0.0, 2.0).with_ui(
                FieldUi::new("settings.llm.field.temperature.label")
                    .described("settings.llm.field.temperature.description"),
            ),
            float_range("topP", 0.95, 0.0, 1.0).with_ui(
                FieldUi::new("settings.llm.field.topP.label").described("settings.llm.field.topP.description"),
            ),
            // `z.number()` sem `.int()` no original — Float, não Int, para não recusar fracionário
            // que o TypeScript aceitava.
            float_range("maxTokens", 4096.0, 256.0, 32768.0).with_ui(
                FieldUi::new("settings.llm.field.maxTokens.label")
                    .described("settings.llm.field.maxTokens.description"),
            ),
            int_range("historyLimit", 20, 1, 100).with_ui(
                FieldUi::new("settings.llm.field.historyLimit.label")
                    .described("settings.llm.field.historyLimit.description"),
            ),
            int_range("maxToolSteps", 15, 1, 50).with_ui(
                FieldUi::new("settings.llm.field.maxToolSteps.label")
                    .described("settings.llm.field.maxToolSteps.description"),
            ),
            // Só o Ollama aceita `num_ctx` por request; o turno ainda corta no máximo do modelo
            // (`/api/show`), então o teto aqui é só o limite do controle.
            int_range("contextWindow", 32768, 2048, 1048576).with_ui(
                FieldUi::new("settings.llm.field.contextWindow.label")
                    .described("settings.llm.field.contextWindow.description"),
            ),
            // `brain::reply_language`: responde no idioma em que a mensagem foi escrita.
            boolean("matchUserLanguage", true).with_ui(
                FieldUi::new("settings.llm.field.matchUserLanguage.label")
                    .described("settings.llm.field.matchUserLanguage.description"),
            ),
            boolean("enableMonologue", false).with_ui(
                FieldUi::new("settings.llm.field.enableMonologue.label")
                    .described("settings.llm.field.enableMonologue.description"),
            ),
            // Default `true`: o resumo rolante sempre rodou (`chat/send.rs`) antes de a chave valer.
            boolean("enableSummarization", true).with_ui(
                FieldUi::new("settings.llm.field.enableSummarization.label")
                    .described("settings.llm.field.enableSummarization.description"),
            ),
            boolean("enableThinking", false).with_ui(
                FieldUi::new("settings.llm.field.enableThinking.label")
                    .described("settings.llm.field.enableThinking.description"),
            ),
            float_range("thinkingMaxTokens", 4096.0, 256.0, 100000.0).with_ui(
                FieldUi::new("settings.llm.field.thinkingMaxTokens.label")
                    .described("settings.llm.field.thinkingMaxTokens.description"),
            ),
            text("systemPrompt", "").with_ui(
                FieldUi::new("settings.llm.field.systemPrompt.label")
                    .control("textarea")
                    .helper("settings.llm.field.systemPrompt.helperText"),
            ),
            choice("embeddingProvider", "local", EMBEDDING_PROVIDER_IDS).with_ui(
                FieldUi::new("settings.embeddings.field.embeddingProvider.label")
                    .control("select")
                    .helper("settings.embeddings.field.embeddingProvider.helperText")
                    .options(&[
                        UiOption {
                            label: "settings.embeddings.field.embeddingProvider.option.local",
                            value: "local",
                        },
                        UiOption {
                            label: "settings.embeddings.field.embeddingProvider.option.google",
                            value: "google",
                        },
                        UiOption {
                            label: "settings.embeddings.field.embeddingProvider.option.openrouter",
                            value: "openrouter",
                        },
                        UiOption {
                            label: "settings.embeddings.field.embeddingProvider.option.ollama",
                            value: "ollama",
                        },
                        UiOption {
                            label: "settings.embeddings.field.embeddingProvider.option.openaiCompatible",
                            value: "openaiCompatible",
                        },
                        UiOption {
                            label: "settings.embeddings.field.embeddingProvider.option.unsloth",
                            value: "unsloth",
                        },
                    ]),
            ),
            text("embeddingModel", "").with_ui(
                FieldUi::new("settings.embeddings.field.embeddingModel.label")
                    .control("text")
                    .helper("settings.embeddings.field.embeddingModel.helperText"),
            ),
            choice(
                "embeddingTaskType",
                "SEMANTIC_SIMILARITY",
                &[
                    "SEMANTIC_SIMILARITY",
                    "CLASSIFICATION",
                    "CLUSTERING",
                    "RETRIEVAL_DOCUMENT",
                    "RETRIEVAL_QUERY",
                    "QUESTION_ANSWERING",
                    "FACT_VERIFICATION",
                    "CODE_RETRIEVAL_QUERY",
                ],
            )
            .with_ui(
                FieldUi::new("settings.embeddings.field.embeddingTaskType.label")
                    .control("select")
                    .helper("settings.embeddings.field.embeddingTaskType.helperText")
                    .options(&[
                        UiOption {
                            label: "Semantic Similarity",
                            value: "SEMANTIC_SIMILARITY",
                        },
                        UiOption {
                            label: "Classification",
                            value: "CLASSIFICATION",
                        },
                        UiOption {
                            label: "Clustering",
                            value: "CLUSTERING",
                        },
                        UiOption {
                            label: "Retrieval Document",
                            value: "RETRIEVAL_DOCUMENT",
                        },
                        UiOption {
                            label: "Retrieval Query",
                            value: "RETRIEVAL_QUERY",
                        },
                        UiOption {
                            label: "Question Answering",
                            value: "QUESTION_ANSWERING",
                        },
                        UiOption {
                            label: "Fact Verification",
                            value: "FACT_VERIFICATION",
                        },
                        UiOption {
                            label: "Code Retrieval Query",
                            value: "CODE_RETRIEVAL_QUERY",
                        },
                    ]),
            ),
            // `z.number().int().min(0)` sem teto — `int_range` exige os dois lados, então o máximo
            // usa `i64::MAX` como "sem teto" para não perder o piso de 0 que o TypeScript exigia.
            int_range("embeddingOutputDimensionality", 0, 0, i64::MAX).with_ui(
                FieldUi::new("settings.embeddings.field.embeddingOutputDimensionality.label")
                    .control("number")
                    .helper("settings.embeddings.field.embeddingOutputDimensionality.helperText"),
            ),
        ],
    }
}

/// `llm-api-keys-config.schema.ts` + o campo `elevenLabs` acrescentado em `llm-config.schema.ts`
/// (`ApiKeysConfigSchema`). São credenciais: o schema é portado normalmente (o store já recusa
/// escrita fora de rota), mas nenhum valor real de chave entra aqui — todo default é string vazia.
fn api_keys() -> Namespace {
    Namespace {
        name: "apiKeys",
        fields: vec![
            text("googleAI", "").with_ui(
                FieldUi::new("settings.llm.field.googleAI.label")
                    .control("password")
                    .helper("settings.llm.field.googleAI.helperText"),
            ),
            text("openrouter", "").with_ui(
                FieldUi::new("settings.llm.field.openrouter.label")
                    .control("password")
                    .helper("settings.llm.field.openrouter.helperText"),
            ),
            text("ollamaBaseUrl", llm_defaults::OLLAMA_BASE_URL).with_ui(
                FieldUi::new("settings.llm.field.ollamaBaseUrl.label")
                    .helper("settings.llm.field.ollamaBaseUrl.helperText"),
            ),
            text("ollamaApiKey", "").with_ui(
                FieldUi::new("settings.llm.field.ollamaApiKey.label")
                    .control("password")
                    .helper("settings.llm.field.ollamaApiKey.helperText"),
            ),
            text("openaiCompatibleBaseUrl", "").with_ui(
                FieldUi::new("settings.llm.field.openaiCompatibleBaseUrl.label")
                    .helper("settings.llm.field.openaiCompatibleBaseUrl.helperText"),
            ),
            // label é OPENAI_COMPATIBLE_PROVIDER.credentialLabel — texto literal do catálogo
            // ("Authorization Key"), não chave de i18n.
            text("openaiCompatibleApiKey", "").with_ui(
                FieldUi::new("Authorization Key")
                    .control("password")
                    .helper("settings.llm.field.openaiCompatibleApiKey.helperText"),
            ),
            // Unsloth Studio/Desktop (`unsloth.ai/docs/basics/api`): OpenAI-compatível, porta típica 8888,
            // chave `sk-unsloth-…` criada em Settings → API. Base URL com `/v1`, como `openaiCompatible`.
            text("unslothBaseUrl", "http://localhost:8888/v1").with_ui(
                FieldUi::new("settings.llm.field.unslothBaseUrl.label")
                    .helper("settings.llm.field.unslothBaseUrl.helperText"),
            ),
            text("unslothApiKey", "").with_ui(
                FieldUi::new("settings.llm.field.unslothApiKey.label")
                    .control("password")
                    .helper("settings.llm.field.unslothApiKey.helperText"),
            ),
            // `elevenLabs` não tem `.describe()` do lado TypeScript — sem UI mesmo.
            text("elevenLabs", ""),
        ],
    }
}

pub fn namespaces() -> Vec<Namespace> {
    vec![llm(), api_keys()]
}
