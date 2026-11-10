//! Módulo `brain` em Rust — o maior do backend (~11,7k LOC no TypeScript) e o que destrava
//! `chat`, `tools` e o laço proativo de `vision`.
//!
//! `modules()` continua `vec![]` de propósito, não por esquecimento: `brain` não possui NENHUM
//! canal IPC próprio no contrato TypeScript (não existe `brain:*` em
//! `desktop-ipc-channels.ts`) — quem tem canal é `chat` (`mika:chat-send`), que desde
//! `6b9bb6ec`/a ligação em `modules/chat.rs` já CHAMA [`turn::run_turn_ollama`] em produção. Este
//! módulo continua existindo como o dono do PIPELINE (o laço de turno, os guardas, a reflexão), não
//! como dono de rota — a diferença entre "vazio = não portado" (o resto do repo) e "vazio = não tem
//! canal para reivindicar" é só deste módulo, e é isso que evita um `Box::new` aqui só para bater
//! com o padrão dos outros arquivos.
//!
//! **Diretório, e não arquivo único, por obrigação:** a regra de 300 linhas por arquivo
//! (`AGENTS.md` §3) não cabe num port deste tamanho, e um arquivo só também impediria mais de uma
//! frente de trabalhar aqui ao mesmo tempo. A divisão segue o pipeline de um turno, que é como o
//! TypeScript já está organizado — não é gaveta arbitrária:
//!
//! - [`turn`] — o laço de um turno (`runSendMessageTurn`): compõe o prompt, chama o provedor,
//!   roda o laço de ferramentas, emite os eventos de streaming. **Ligado** (ver acima).
//! - [`guards`] — o que decide se a Mika DEVE falar: repetição, rastreio de assunto. Ainda não
//!   ligado a `turn`/`chat`.
//! - [`reflection`] — a reflexão que roda a cada 5º turno. **Ligada** desde esta rodada, por
//!   [`post_turn::spawn_post_turn_listener`] (ver abaixo).
//! - [`post_turn`] — o resto do que rodava DEPOIS do turno no TypeScript (`PostTurnPipeline`,
//!   `brain/chat/pipeline.ts`): assina `ThinkingStart`/`StreamChunk`/`ThinkingStop` do barramento
//!   (publicados por `chat::send`, fora do escopo desta rodada), reconstrói o par usuário/Mika e
//!   dispara memória episódica + [`reflection`] no mesmo gatilho. Precisa de um `spawn` no boot —
//!   ver o cabeçalho de `post_turn::listener` para a linha exata.
//! - `persona` (`pub(crate)`, só [`turn`] chama para compor o prompt) — o `system.md` rico (15
//!   placeholders), fechando a lacuna "Sem `prompts/system.md`" que o cabeçalho de `turn.rs`
//!   documentava. Visibilidade elevada de privado para `pub(crate)` nesta rodada só para
//!   `modules/mod.rs::register_config_bound_tools` poder chamar
//!   `persona::screen_context::spawn()` no boot — nenhum outro item do módulo ganhou alcance novo.
//!   Ver o cabeçalho de `persona/mod.rs`.
//! - [`thought`] — `ThoughtService::handleProactiveEvent` (TS): consome
//!   `EventKey::BrainProactiveThought` e entrega ao [`awareness::engine::ProactiveEngine`]
//!   injetado. Precisa de um `spawn` no boot, encadeado com `awareness::spawn_proactive_engine`
//!   (que agora devolve o `Arc` que este módulo consome) — ver o cabeçalho de [`thought`] para a
//!   linha exata.
//!
//! Os seis se encontram aqui, e só aqui; um não importa o outro pelas costas.
//!
//! ## `tools`/`settings` — as 5 tools de `BrainFeature` e a aba `brain`
//!
//! Nova frente nesta rodada: [`tools`] registra as 5 tools de `BrainFeature`
//! (`brain.feature.ts`) no `ToolRegistry` nativo (tabela de paridade completa no cabeçalho de
//! `tools/mod.rs`), e [`settings`] registra a aba `brain` no `FeatureRegistry`. As duas são
//! INDEPENDENTES do pipeline de turno acima — não tocam `guards`/`reflection`/`turn`.
//!
//! **A aba `llm`, apesar de `BrainFeature.settings = [BRAIN_SETTINGS, LLM_SETTINGS]` no TS
//! agregá-la aqui, é registrada por `modules/llm/`, não por este módulo** — coordenado em
//! 2026-09-21 depois de uma colisão de `tabId: "llm"` entre as duas frentes (o `FeatureRegistry`
//! dedupe por `tabId` descartando a segunda em silêncio). Quem AUTORA o schema no TS é
//! `services/llm/llm.schema.ts`; `brain.feature.ts` só importa e agrega — `modules/llm/` segue
//! essa autoria. Ver `settings/mod.rs` para o achado completo.
//!
//! 4 das 5 tools e a aba de settings não precisam de `ConfigStore`: registram como efeito
//! colateral de [`modules()`], mesmo padrão de `gamer::modules()`. Só `web` (gate `browser.enabled`)
//! precisa — por isso [`register`] existe separado, para a segunda passada de boot.

pub mod awareness;
/// Detector de crise (autolesão) na entrada do turno — `turn::compose_system_prompt` e `chat::send`.
pub(crate) mod crisis;
pub mod guards;
pub mod semantic_repeat;
/// `pub(crate)`, não privado: `modules::vision::llm_client` reusa [`llm_provider::build_remote_model`]
/// (mesma guarda SSRF + modo offline e o mesmo `OpenAI::chat()` — Chat Completions, não Responses) para
/// o caminho `vision.provider == "openaiCompatible"` — sem isto, o vision teria de duplicar a
/// construção do cliente remoto inteira. Mesmo precedente de elevação de `persona` documentado
/// acima, escopo igualmente contido: só este módulo ganhou alcance novo nesta rodada.
pub(crate) mod llm_provider;
pub mod ollama_warmup;
pub(crate) mod persona;
pub(crate) mod provider_params;
pub mod post_turn;
mod prompt_loader;
pub(crate) mod reply_language;
pub mod reflection;
mod response_json;
mod settings;
pub mod thought;
mod tools;
pub mod turn;

use std::path::PathBuf;
use std::sync::Arc;

use crate::config::store::ConfigStore;
use crate::native::NativeModule;

/// Registra as 4 tools sem config e a aba de settings `brain` como efeito colateral — nenhuma
/// reivindica canal de IPC (ver cabeçalho acima; a aba `llm` é de `modules/llm/`, não daqui).
pub fn modules() -> Vec<Box<dyn NativeModule>> {
    tools::register_config_free_tools();
    settings::register();
    vec![]
}

/// A tool `web` precisa do `ConfigStore` do processo (gate `browser.enabled`) — chame da segunda
/// passada de boot, mesmo ponto que já liga `vision::register`
/// (`modules/mod.rs::register_config_bound_tools`). `chat_db_path` entrou nesta rodada (além de
/// `store`): `content`'s `daily_recap` lê `chat.db` para o recap, e o caminho já vem RESOLVIDO por
/// quem chama (o único ponto com `AppHandle`) — `None` quando a resolução falhou no boot, e
/// `daily_recap` degrada nomeado em vez de este módulo precisar de `AppHandle` também (ver
/// `tools::register_config_bound_tools` e `tools/content.rs::ContentHandler`).
pub fn register(store: Arc<ConfigStore>, chat_db_path: Option<PathBuf>) {
    tools::register_config_bound_tools(store, chat_db_path);
}
