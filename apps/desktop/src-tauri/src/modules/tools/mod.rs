//! Módulo `tools` em Rust — port do backend TypeScript correspondente.
//!
//! **Os 4 canais deste diretório saíram do sidecar em 2026-09-22**: `mika:tools-list`,
//! `mika:tools-list-by-plugin`, `mika:tool-execute` e `tool-approval-response` são servidos por
//! `native.rs`. Ver `docs/tauri-rust-migration.md` e o registro em `native/mod.rs`.
//!
//! Virou diretório em 2026-09-21c só pela regra de 300 linhas do AGENTS.md: `registry.rs` carrega
//! o `ToolRegistry` em si (o alicerce que módulos já portados usam para se cadastrar),
//! `approval.rs` o gate de aprovação (`ApprovalGateService`), `access.rs` a política de quem pode
//! pedir o quê, `catalog.rs` o achatamento do schema para a UI e `native.rs` os quatro canais.
//! Aqui fica a MEDIÇÃO — o denominador do catálogo e o que dele ainda degrada — e os testes que a
//! ancoram contra o fonte real, para que ela não envelheça calada se o fonte mudar de forma.
//!
//! ## Os 4 canais: o que cada um faz
//!
//! - `mika:tool-execute` (`ipc/tools.ts`) chama `toolRegistry.execute(name, args)`: invoca o
//!   handler async registrado por CADA tool (funções reais de `brain`, `voice`, `vision`,
//!   `browser`, `embeddings` e plugins dinâmicos).
//! - `mika:tools-list`/`-by-plugin` (`ipc/tools.ts` → `tool-access.ts::mapToolsByPlugin`) leem
//!   `toolRegistry.getAll()` — o mesmo registro, populado no boot. `-by-plugin` FILTRA o mesmo
//!   registro, não é fonte própria — herda a completude (ou incompletude) de `tools-list` inteira.
//!
//! ## Remedido 2026-09-21e: o catálogo nativo foi de 6 para 14 tools — ainda incompleto, agora com
//! ## uma lacuna nomeada (não mais "dezenas contra 6")
//!
//! Medido de novo por grep (`rg "tools::registry\(\)\.register"`) MAIS leitura de cada `modules()`/
//! `register()` que `lib.rs::setup`/`modules::register_config_bound_tools` de fato chamam — não só
//! o que existe no fonte, o que RODA. 14 tools nativas hoje, não mais 6:
//!
//! - Config-livres (efeito colateral de `modules()`, sem `AppHandle`): `track_gamer_mood`, `steam`
//!   (`gamer::register_tools`), `mood`, `desktop` (`features::modules`), `match_effect`,
//!   `memory_console`, `collaboration` (`brain::tools::register_config_free_tools`, via
//!   `brain::modules()`).
//! - Config-vinculadas (`register_config_bound_tools`, chamada por `lib.rs::setup`):
//!   `vision_capture`, `vision_read` (`vision::register`), `web`, `content` (as duas de
//!   `brain::tools::register_config_bound_tools` — `content` fecha sobre persona/idioma do prompt,
//!   por isso NÃO é config-livre apesar da rodada anterior ter listado assim; `brain/tools/mod.rs`
//!   já documentava a dependência certa, só este cabeçalho estava desalinhado com ela),
//!   `avatar` (`features::character::register`), `user_profile` (`features::user::register`),
//!   `reminder` (`features::scheduler::register`).
//!
//! O catálogo TS também foi remedido linha a linha (não só citado de memória) e mudou de forma: 24
//! tools no total — `mika.embeddings` e `mika.browser` (`browser.feature.ts`: `tools: {}`) não têm
//! NENHUMA, ao contrário do que a rodada anterior presumia ao dizer "muito mais". `mika.brain` (5) e
//! `mika.vision` (2) — as duas únicas frentes-núcleo com tool — estão com paridade de NOME/despacho
//! TOTAL: toda tool que o TS delas declara já valida e despacha nativamente (3 das 5 de `brain`
//! ainda degradam nomeado por dependerem de LLM sem porte — `brain/tools/mod.rs` já documenta isso
//! como paridade PARCIAL de comportamento, não de existência). O que falta, medido contra o TS:
//!
//! ## Remedido em 2026-09-22: 24 de 24 por NOME — e o veredito NÃO muda
//!
//! Cinco frentes fecharam o buraco inteiro da tabela anterior: `voice`, `person_catalog`, as 3 de
//! `privacy` (inclusive a destrutiva), as 4 de `gamer-companion` e `get_system_info`. **Zero nomes
//! faltando.**
//!
//! `recommend_media` foi a última, e derrubou junto a premissa que a segurava: o cabeçalho de
//! `modules/gamer/` afirmava que não havia cliente de LLM alcançável dali. Era falso —
//! `turn::run_turn_ollama_raw` sempre esteve, e `brain/tools/content.rs` já provava isso. Premissa
//! escrita num cabeçalho envelhece igual a comentário: vale enquanto alguém remede.
//!
//! **E mesmo com 24 de 24, os canais de tool continuam no sidecar.** Porque paridade de NOME não é
//! paridade de COMPORTAMENTO, e é o comportamento que o usuário perde.
//!
//! ## Medição ação a ação — 2026-09-22
//!
//! A tabela que ficava aqui listava oito tools como degradando, e a remedição refutou **todas as
//! oito**, uma a uma: `web` (reqwest+scraper reais), `content` (lê o `chat.db` de verdade),
//! `collaboration`, `memory_console` (a busca episódica do TS nunca usou embedding — é por palavra-
//! chave), `voice`, `person_catalog` (11 de 11 ações, com PII redigida antes de gravar),
//! `list_memories` e `game_session`/`game_knowledge` (os dois chamam `run_turn_llm_raw`). Todas
//! foram escritas como verdade e envelheceram em dias — cabeçalho envelhece igual a comentário.
//!
//! **O TS declara 71 ações; o Rust despacha 60 (84,5%). 19 das 24 tools estão 100% portadas.**
//! Conta como despacho o braço que executa lógica real contra serviço/porta real, mesmo que sob
//! condição de runtime deságue em erro nomeado — é o que o próprio TS faz. NÃO conta o braço que
//! devolve o mesmo erro para qualquer entrada, sem tentar nada.
//!
//! O que ainda degrada não é nada do que a tabela antiga dizia:
//!
//! | Tool | Ações que NÃO despacham | Causa |
//! |---|---|---|
//! | `vision_read` | `describe`, `ocr`, `people` (degrada inteira) | sem chamada de LLM multimodal em Rust |
//! | `vision_capture` | `screenshot`, `watch_now` (só `frame` despacha) | mesma causa |
//! | `user_profile` | `set`, `forget`, `set_personality` | `no_social()` fixo em `features/user/tool.rs` |
//! | `desktop` | `launch`, `window` | `Err(...)` fixo em `features/desktop/tool.rs` |
//! | `steam` | `achievements` | não existe em schema Rust nenhum |
//!
//! Divergências de FORMA, com a ação despachando: `voice.transcribe` só aceita `audio/pcm*` (o TS
//! decodifica mp3/wav/ogg/webm);
//! `clear_history` cobre 11 dos 18 sub-stores e `export_data` 2 dos 8 — os dois nomeiam os que
//! faltam na resposta.
//!
//! **A decisão mudou em 2026-09-22, e o que a mudou não foi a tabela ficar vazia.** O argumento
//! anterior era que reivindicar trocaria função que FUNCIONA no Electron por recusa nomeada. Ele
//! valia enquanto o sidecar fosse uma alternativa; deixou de valer quando o objetivo passou a ser
//! APAGÁ-LO. Enquanto estes 4 canais descessem para o Node, o sidecar tinha de continuar vivo —
//! e com ele TODO o resto que ele serve. A escolha real não era "Rust incompleto × Electron
//! completo", era "Rust incompleto × manter o processo inteiro por causa de um punhado de ações".
//!
//! As ações que faltavam foram atacadas uma a uma em vez de servirem de motivo para adiar; o que
//! sobrar delas está na tabela acima, nomeado, e continua degradando com erro explícito — que é o
//! que o próprio TS faz quando a peça de baixo não responde.
//!
//! **O que remedir a cada rodada**, e como, para não repetir o erro das anteriores: contar NOME
//! registrado não responde a pergunta. A medição que vale é, por tool, quantas AÇÕES despacham de
//! verdade contra quantas o TS declara — a tabela acima, refeita. Duas vezes um cabeçalho deste
//! repositório afirmou lacuna que já não existia.
//!
//! `brain::turn.rs` (`mika:chat-send`) já consumia este mesmo registro em produção antes destes
//! canais — `mika:tool-execute`/`mika:tools-list` são um contrato DIFERENTE (UI de settings e
//! chamada direta de tool fora do loop do LLM), com política de acesso própria (`access.rs`), por
//! isso não bastava "já executa em algum lugar". `mika:tools-list-by-plugin` filtra o mesmo
//! registro e herda o que ele tiver.
//!
//! Os testes abaixo ancoram o denominador (o fonte TS) e o que JÁ existe (positivo, ao vivo). Os
//! pins negativos que mediam a lacuna saíram: asserção de ausência contra um `OnceLock` de processo
//! com `cargo test` em paralelo passa por ausência de vizinho, não por ausência de porte — ver o
//! comentário de bloco em `tests.rs`.
//!
//! ## `tool-approval-response` — por que os quatro migram juntos, sempre
//!
//! É um canal de EVENTO (`mika_send`, fire-and-forget do lado do renderer —
//! `DESKTOP_LOCAL_EVENT_ROUTES.toolApprovalResponse` em `ipc/tool-approval.ts`), não um `invoke`.
//!
//! O gate de aprovação de hoje, do lado Node:
//! 1. `registerToolsIPC()` (`ipc/tools.ts`) monta UM `ApprovalHandler` (`createApprovalHandler`,
//!    `ipc/tool-approval.ts`) e o instala em `toolRegistry.setApprovalHandler(...)`.
//! 2. Toda chamada de `mika:tool-execute` passa por `ensureBrokerApproval` ANTES de executar a
//!    tool: se `isHighRiskCall(name, args)` for verdadeiro e a tool não for `destructive` (que já
//!    tem seu próprio gate), ele chama `toolRegistry.requestApproval(...)`.
//! 3. `requestApproval` acha a janela certa (`pickApprovalWindow` — nunca `settings`/`plugin-view`
//!    se houver uma janela "normal" viva), manda `ToolApprovalRequest` pra ela e registra a
//!    aprovação pendente num `Map` keyed por `toolCallId`, com um backstop de timeout
//!    (`TOOL_APPROVAL_BACKSTOP_MS`).
//! 4. É `tool-approval-response` — o mesmo evento deste canal — quem RESOLVE essa entrada do `Map`
//!    quando o usuário decide, casando pelo `toolCallId` e pelo `webContentsId` de quem perguntou.
//!
//! Ou seja: quem CRIA a aprovação pendente é o handler de `mika:tool-execute`, e é o MESMO processo
//! que precisa receber a resposta. `bridge.rs` roteia por canal inteiro — um canal é nativo OU vai
//! pro sidecar, nunca os dois (`mika_send`, `state.native.owns(&channel)`). Reivindicar só
//! `tool-approval-response`, com `tool-execute` do outro lado, interceptaria a resposta de TODA
//! aprovação que o outro processo está esperando: o usuário clica aprovar/negar, o evento nunca
//! chega no `Map` dele, e a chamada fica pendurada até o backstop de timeout estourar — pior do que
//! não migrar, não equivalente. **Por isso os quatro migraram juntos, e por isso o teste
//! `claims_all_four_tool_channels_or_none` cobra os quatro:** a invariante vale nos dois sentidos,
//! e quem um dia devolver um deles ao outro lado reabre exatamente este buraco.
//!
//! Do lado Rust o par é `native.rs::execute` (cria a pendência, via `ensure_broker_approval` →
//! `ToolRegistry::request_approval`) e `native.rs::resolve_approval` (resolve, via
//! `window_approval::installed()`). O `OnceLock` de `window_approval.rs` existe só para fechar esse
//! par: `set_approval_handler` guarda o TRAIT, que sabe `decide()` e não `resolve()`.
//!
//! **Como o gate falha, hoje (TS, verificado por leitura de `ipc/tool-approval.ts` e
//! `ipc/tool-approval.ts::ensureBrokerApproval`):** fecha para o lado seguro nos dois pontos que
//! importam. Sem janela viva para perguntar, `createApprovalHandler` devolve
//! `{ approved: false, scope: 'once' }` sem sequer abrir a pergunta (`pickApprovalWindow` retornando
//! `null`). E toda decisão que não seja `approved: true` vira `{ success: false, error: 'user_denied' }`
//! em `ensureBrokerApproval` — não há caminho onde ausência de resposta, erro ou timeout resultem em
//! execução. Os testes abaixo ancoram essas duas formas contra o fonte, para que qualquer mudança
//! que afrouxasse esse fail-closed (ex.: trocar o `false` do "sem janela" por um `true` "por
//! segurança") derrube este arquivo em vez de passar despercebida. `registry.rs` porta o MESMO
//! fail-closed do lado Rust — `ApprovalGateService::request_approval` (`approval.rs`) nega sem
//! handler configurado, e `ToolRegistry::execute` (`registry.rs`) aplica esse gate a qualquer tool
//! registrada via `register_destructive`, tanto para `brain::turn` (via `mika:chat-send`) quanto
//! para `mika:tool-execute`.
//!
//! `native.rs` porta a OUTRA metade, a que gateia por risco da CHAMADA e não pela marca da tool
//! (`ensure_broker_approval`), com dois fail-closed próprios que os testes fixam: `approved`
//! ausente ou ilegível é NEGAÇÃO, e `scope` desconhecido é `Once` — o mais restrito. Um payload
//! malformado não pode virar aprovação permanente.
//!
//! **Ligado desde 2026-09-21, completado em 2026-09-22:** a última linha de
//! `modules::register_config_bound_tools` chama `tools::install_approval_handler(app.clone())`, e
//! `modules/mod.rs::the_boot_actually_calls_the_config_bound_registration` ancora que o boot chama
//! aquela função. `install` (e não `set_approval_handler` direto) porque o handler precisa ficar
//! alcançável para `resolve()` — sem isso o diálogo apareceria e a resposta não chegaria em
//! ninguém: toda aprovação penduraria 30s e negaria. O gate deixou de negar por padrão porque
//! existe handler, não porque afrouxou.
//!
//! **Fechado:** `modules/mcp/register.rs` trocou para `.register_destructive(...)` e despacha de
//! verdade contra o servidor MCP quando aprovado — a lacuna nomeada aqui numa rodada anterior.

mod access;
mod approval;
mod catalog;
mod native;
pub mod outcome;
mod registry;
mod validation;
mod window_approval;

// Só o que módulos já portados (`brain::turn`, `chat::send`, `gamer`, `features`, `vision`) de fato
// consomem hoje. `ApprovalHandler`/`ApprovalRequest`/`ApprovalDecision`/`ApprovalScope` (o TRAIT e
// seus tipos) seguem sem reexport — ninguém fora de `modules/tools/` precisa nomeá-los ainda; quem
// instala o handler real só precisa de `WindowApprovalHandler` (abaixo), que já implementa o trait
// por baixo. `WindowApprovalHandler` é o handler real, instalado no boot — ver o parágrafo acima.
pub use registry::{registry, ToolDefinition, ToolHandler, ToolOrigin};
// `WindowApprovalHandler` NÃO é reexportado: quem instala chama `install_approval_handler`, que
// faz as duas metades (registro + alcance para `resolve`). Expor o construtor ao lado convidaria a
// instalar pela metade de novo — e essa metade some sem erro nenhum, só com o clique do usuário
// indo para lugar nenhum.
pub use window_approval::install as install_approval_handler;
// Só o disparo sintético para `modules::e2e_hooks` — não o handler nem `ApprovalRequest`, mesma
// disciplina do reexport acima (a metade exposta é a mínima que o chamador de teste precisa).
#[cfg(any(test, feature = "e2e"))]
pub use window_approval::trigger_e2e_approval_request;

use crate::native::NativeModule;

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(native::ToolsModule)]
}

// Suíte em arquivo separado só pela regra de 300 linhas do AGENTS.md (mesma razão que já fez este
// diretório existir — ver o cabeçalho acima) — continua sendo o mesmo módulo `tools::tests`.
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
