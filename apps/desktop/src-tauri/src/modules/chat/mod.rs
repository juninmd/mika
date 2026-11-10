//! Módulo `chat` em Rust — port de `apps/desktop/src/main/services/chat/*` (IPC de conversa) e
//! do backend que aquele IPC invoca.
//!
//! **Os 4 canais do grupo `mika:chat-*` reivindicados nesta rodada.** A terceira onda (`6b9bb6ec`)
//! já tinha entregue `mika:chat-send` sobre `brain::turn::run_turn_ollama`. Os outros 3 ficavam
//! presos porque `chat-send` passava `history: vec![]` sempre e não havia onde guardar um
//! `TurnAbortHandle` entre chamadas.
//!
//! **`chat-history`/`chat-clear` — [`history_store::HistoryStore`].** O TS mantém `this.messages`
//! EM MEMÓRIA como fonte de verdade (`history-state.ts`); o banco (`@libsql/client`) é só
//! persistência. Desde que `chat-send` passou a ser resolvido em Rust, aquele array do processo
//! Node nunca mais é escrito por uma conversa real — `chat-history` lendo de lá devolveria sempre
//! a mesma foto congelada do dia em que a migração começou. `history_store::HistoryStore` é a
//! sessão implícita equivalente do lado Rust, dentro do PRÓPRIO `ChatModule` (mesmo padrão que
//! `voice::VoiceModule` já usa para `mic: Mutex<MicStateBreaker>` — estado que vive pela vida do
//! app porque `ChatModule` é construído uma vez em `modules()` e fica dentro de
//! `ShellState.native`, sem precisar editar `state.rs`).
//!
//! **Persistida desde esta rodada (`chat_db.rs`, rusqlite).** A dívida "histórico em memória,
//! perdido no restart" registrada no cabeçalho de `scripts/checks/sidecar-attrition.mts` está
//! fechada: `ChatModule.invoke` chama `HistoryStore::ensure_persistence(app)` antes de
//! `chat-send`/`chat-history`/`chat-clear`, o que abre `chat.db` (o MESMO arquivo que
//! `bootstrapChat` já usa — `db.getDatabase('chat.db', 'plugins/mika.chat')`) e restaura a sessão
//! mais recente. `session-manager.ts` (múltiplas sessões nomeadas) segue não portado — uma única
//! sessão "atual" por processo, mas agora ELA sobrevive ao restart. Ver o cabeçalho de
//! `chat_db.rs` para a decisão rusqlite×libsql e a compatibilidade com o arquivo do usuário.
//!
//! **`chat-cancel` — [`turns`], o registro de turno por sessão.** `brain/turn.rs` já prova, em
//! teste, que `TurnAbortHandle::abort` cancela a chamada em voo em vez de deixar uma segunda
//! resposta escapar; faltava só onde guardar o handle do turno EM CURSO entre a chamada de
//! `chat-send` que o criou e uma chamada de `chat-cancel` SEGUINTE. `turns::registry()` é um
//! `Mutex<HashMap<window_label, TurnAbortHandle>>` ÚNICO do processo — um registro por janela
//! porque `is_owner_surface` permite mais de uma (`main`, `settings`, `wizard`) chamar `chat-send`.
//! Ele não vive dentro de `ChatModule` como `HistoryStore`: o barge-in, em `modules/voice`, precisa
//! abortar o turno de fora, e registro privado deixaria a detecção funcionando sem poder atuar. Ver
//! o cabeçalho de `turns.rs` para o resto.
//!
//! **Histórico de verdade chegando a `run_turn_ollama`.** `turn.rs` já aceitava `history` como
//! parâmetro (todo chamador passava `vec![]` — ver o cabeçalho dele, "Sem memória entre turnos").
//! `send::handle_chat_send` agora monta esse vetor a partir de `HistoryStore::to_turn_history`
//! ANTES do turno, e grava o par usuário/assistente DEPOIS de um turno bem-sucedido. Nada mudou em
//! `turn.rs` — só passou a existir quem preenche o parâmetro que já existia.
//!
//! **Escopo menor que o TS, herdado de `turn.rs` — não escondido:** sem anexos/visão. (O
//! `prompts/system.md` rico já é o prompt do turno — ver `brain/persona/mod.rs`.) Um turno que
//! FALHA não é gravado no histórico (o usuário pode reenviar; um erro não é uma fala real) — ver
//! o cabeçalho de `turn.rs` para o detalhe de cada corte herdado.
//!
//! **Chokepoint de ESCRITA (`append_message`) e ponte para o renderer (`spawn_assistant_message_
//! bridge`), ambos desta rodada.** `brain::tools::collaboration_session` precisa gravar cada turno
//! de um debate na MESMA sessão "atual" que `HistoryStore` grava via `mika:chat-send` — sem
//! alargar o campo privado `HistoryStore` (que só existe depois do primeiro `mika:chat-*` real, ver
//! `history_store.rs`), `append_message` abre a própria `ChatDb` sobre o MESMO arquivo, mesmo
//! desenho de `recent_messages` só que para escrita (ver `chat_write.rs` para a retentativa de
//! `seq` que essa segunda conexão exige). `spawn_assistant_message_bridge` fecha a outra metade:
//! `EventKey::AssistantMessage` (recap/debate) publicado no barramento de processo só chega à tela
//! se alguém encaminhar para a janela — ver `assistant_bridge.rs`.

mod assistant_bridge;
mod chat_db;
mod chat_recap;
mod chat_write;
mod history;
mod history_store;
mod persona_composer;
mod persona_relationship;
pub(crate) mod report;
// `pub(crate)`, não privado: `brain::awareness::spawn_proactive_engine` (fora deste módulo) chama
// `proactive::send_awareness` a cada candidato vencedor do tick — ver o cabeçalho de `proactive.rs`.
pub(crate) mod proactive;
pub(crate) mod proactive_novelty;
pub(crate) mod prompt;
mod retention;
mod rolling_summary;
mod send;
mod session_adoption;
mod turn_error;
// `pub(crate)`, não privado: `turns::abort_turn` é o hook que `modules/voice/vad_audio.rs` chama
// de FORA deste módulo quando o barge-in dispara — ver o cabeçalho de `turns.rs`.
pub(crate) mod turns;
mod wizard_react;

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use serde_json::Value;
use tauri::AppHandle;

use crate::native::NativeModule;
use crate::protocol::InvokeContext;

pub use history::ChatMessage;
// `brain::persona::compose_system_prompt` injeta humor/vínculo no `system.md` rico — ver o
// cabeçalho de `persona_composer.rs`.
pub(crate) use persona_composer::{gather_persona_state, inject_persona_state};

// Sem chamador FORA deste módulo ainda pela rota de produção normal (só
// `modules/mod.rs::register_config_bound_tools`, o único ponto com `AppHandle`) — mesma categoria
// de re-export direto que `history::ChatMessage` já usa.
pub(crate) use assistant_bridge::spawn_assistant_message_bridge;

// `brain::persona::system_prompt` (o `system.md` rico) precisa das duas — ver o cabeçalho de
// `brain/persona/mod.rs` para o porquê de morarem em `chat` e não em `brain`.
pub(crate) use persona_relationship::{build_relationship_block, effective_relationship_level, relationship_label};
pub(crate) use prompt::{init_prompts_root, load_prompt_no_handle};

/// Chokepoint de LEITURA para fora do módulo `chat` — hoje só `brain::tools::content` (daily
/// recap) chama isto. `HistoryStore` (a sessão "atual" em memória) é campo PRIVADO de `ChatModule`
/// e só existe depois do primeiro `mika:chat-*` real; em vez de expor esse campo, ou de mover o
/// dono do histórico para fora do módulo, abrimos uma conexão de leitura PRÓPRIA sobre o MESMO
/// `chat.db` (WAL torna leitura concorrente com `HistoryStore` segura — ver `chat_db.rs`). Sem
/// escrita aqui: quem grava histórico continua sendo só `send::handle_chat_send` via
/// `HistoryStore`.
pub fn chat_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    chat_db::db_path(app)
}

/// Últimas `limit` mensagens da sessão "atual", ordem cronológica — porte do `.slice(-50)` que
/// `ContentPipelineService.generateDailyRecap` aplica sobre `HistoryService.getAll()`
/// (`content-pipeline.service.ts`). Ver `chat_db.rs::recent_messages` para o bound da query.
pub fn recent_messages(db_path: &Path, limit: i64) -> Result<Vec<ChatMessage>, String> {
    let db = chat_db::ChatDb::open(db_path)?;
    let session_id = db.current_session_id()?;
    chat_recap::recent_messages(&db, &session_id, limit)
}

/// Toda a conversa da sessão "atual", cronológica — `features::privacy` (exportar: direito de
/// acesso/portabilidade). Mesma conexão-própria de leitura de [`recent_messages`], sem o corte.
pub fn all_messages(db_path: &Path) -> Result<Vec<ChatMessage>, String> {
    let db = chat_db::ChatDb::open(db_path)?;
    let session_id = db.current_session_id()?;
    Ok(db.restore(&session_id)?.0)
}

/// Chokepoint de ESCRITA para fora do módulo `chat` — hoje só `brain::tools::collaboration_session`
/// chama isto, um turno de debate por vez. Mesma conexão-própria de `recent_messages` (acima), só
/// que gravando: abre `chat.db`, resolve a sessão "atual" e grava `role`/`content` com `id`/
/// `timestamp` gerados em `chat_write.rs` — ver o cabeçalho de lá para a retentativa de `seq` que
/// essa segunda conexão sobre o MESMO arquivo exige.
pub fn append_message(db_path: &Path, role: &str, content: &str) -> Result<(), String> {
    let db = chat_db::ChatDb::open(db_path)?;
    let session_id = db.current_session_id()?;
    let written = chat_write::append_message(&db, &session_id, role, content)?;
    // Conexão própria, só disco: sem o espelho a memória do processo (histórico da UI, contexto do
    // próximo turno) nunca veria esta linha.
    history_store().mirror_external(&session_id, written);
    Ok(())
}

/// Os 4 canais do grupo `mika:chat-*` — nenhum mais fica no sidecar. Ver o cabeçalho do arquivo.
const CHANNELS: [&str; 6] =
    ["mika:chat-send", "mika:chat-history", "mika:chat-clear", "mika:chat-cancel", "mika:wizard-react", "mika:chat-report"];

/// `HistoryStore` do processo — MESMO padrão de `person_catalog::store_singleton::catalog_store` e
/// `memory::memory_store`: um só `OnceLock`, para `features::privacy::clear` (`clear_history`)
/// nunca abrir uma segunda instância. Duas instâncias independentes fariam `mika:chat-clear`
/// esvaziar a sessão em memória de UMA delas enquanto a outra (a que `ChatModule` de fato usa em
/// `mika:chat-history`) continuaria mostrando as mensagens antigas — a MESMA classe de bug que
/// `store_singleton.rs` já documentou para o catálogo de pessoas. `HistoryStore::new()` não precisa
/// de `AppHandle` (só memória até `ensure_persistence` rodar), então o `OnceLock` é inicializado
/// direto, sem `Result`.
static HISTORY_STORE: OnceLock<Arc<history_store::HistoryStore>> = OnceLock::new();

fn history_store() -> Arc<history_store::HistoryStore> {
    Arc::clone(HISTORY_STORE.get_or_init(|| Arc::new(history_store::HistoryStore::new())))
}

/// Short-term memory at a glance (memory panel): messages held in this session and the rolling
/// summary of what already scrolled out of the recent window.
pub(crate) fn short_term_view() -> (usize, Option<String>) {
    let store = history_store();
    (store.all().map(|m| m.len()).unwrap_or(0), store.rolling_summary().ok().flatten())
}

/// Chokepoint de LIMPEZA para fora do módulo `chat` — só `features::privacy::clear` chama isto.
/// A sessão corrente E as outras gravadas em `chat.db`, na MESMA instância que `mika:chat-clear` usa
/// (`history_store()`), não numa conexão própria: ver o cabeçalho de `HISTORY_STORE`. Sem
/// `ensure_persistence` antes, nada tocaria o disco se `mika:chat-*` ainda não tivesse rodado nesta
/// sessão — uma conversa anterior ficaria intacta em `chat.db` enquanto a tool responde sucesso.
pub fn clear_all_history(app: &AppHandle) -> Result<(), String> {
    let history = history_store();
    history.ensure_persistence(app)?;
    history.clear_all()
}

struct ChatModule {
    // `Arc`, não posse direta: 04-memory.md §"Curto prazo" — o resumo rolante roda em segundo plano
    // (`tauri::async_runtime::spawn`, ver `send.rs`), que exige uma referência `'static`. `&self`
    // não sobrevive além da chamada de `invoke`; um clone do `Arc` sim.
    history: Arc<history_store::HistoryStore>,
}

#[async_trait]
impl NativeModule for ChatModule {
    fn name(&self) -> &'static str {
        "chat"
    }

    fn channels(&self) -> &'static [&'static str] {
        &CHANNELS
    }

    async fn invoke(&self, app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) -> Result<Value, String> {
        match channel {
            "mika:chat-send" => {
                self.history.ensure_persistence(app)?;
                send::handle_chat_send(app, ctx, payload, &self.history, turns::registry()).await
            }
            "mika:chat-history" => {
                self.history.ensure_persistence(app)?;
                history::handle_chat_history(ctx, &self.history)
            }
            "mika:chat-clear" => {
                self.history.ensure_persistence(app)?;
                history::handle_chat_clear(ctx, &self.history)
            }
            "mika:chat-cancel" => turns::handle_chat_cancel(ctx, turns::registry()),
            wizard_react::CHANNEL => wizard_react::handle(app, ctx, payload).await,
            report::CHANNEL => report::handle(app, ctx, payload).await,
            other => Err(format!("canal '{other}' declarado por 'chat' mas não tratado")),
        }
    }
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(ChatModule { history: history_store() })]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-chat-mod-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Prova o chokepoint público (`recent_messages`) de ponta a ponta: abre um `chat.db` num
    /// arquivo real, grava direto pela conexão interna (`ChatDb`, visível dentro deste módulo) e lê
    /// de volta pela MESMA função que `brain::tools::content` vai chamar — sem depender de
    /// `AppHandle` (que este crate não pode forjar em teste, ver `history_store.rs`).
    #[test]
    fn recent_messages_reads_back_what_was_written_to_the_same_chat_db_file() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");

        let session_id = {
            let db = chat_db::ChatDb::open(&path).unwrap();
            let session_id = db.current_session_id().unwrap();
            db.append(&session_id, 0, &ChatMessage { id: "u1".into(), role: "user".into(), content: "oi".into(), timestamp_ms: 1 })
                .unwrap();
            session_id
        };

        let messages = recent_messages(&path, 50).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "oi");

        // Mesma sessão "atual" que a leitura direta via `ChatDb` resolveria — prova que o wrapper
        // não inventa outra sessão por engano.
        let db = chat_db::ChatDb::open(&path).unwrap();
        assert_eq!(db.current_session_id().unwrap(), session_id);
    }

    /// Prova o chokepoint público de ESCRITA (`append_message`) de ponta a ponta: grava pela
    /// função que `brain::tools::collaboration_session` vai chamar e lê de volta pela conexão
    /// interna (`ChatDb::restore`) — as duas pontas de `chat_write.rs` sem precisar de `AppHandle`.
    #[test]
    fn append_message_writes_through_and_recent_messages_reads_it_back() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");

        append_message(&path, "user", "qual o tema do debate?").unwrap();
        append_message(&path, "assistant", "vamos falar de RPG de mesa").unwrap();

        let messages = recent_messages(&path, 50).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "qual o tema do debate?");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "vamos falar de RPG de mesa");
    }

    /// Os 4 canais do grupo `mika:chat-*` existem no contrato — um literal errado aqui não pode
    /// passar sem ser notado.
    #[test]
    fn channels_exist_in_the_ts_contract() {
        let channels_contract = include_str!("../../../../../../packages/sdk/src/contracts/desktop-ipc-channels.ts");

        for channel in CHANNELS.iter() {
            assert!(
                channels_contract.contains(&format!("'{channel}'")),
                "canal {channel} não existe em desktop-ipc-channels.ts — literal errado"
            );
        }
    }

    /// Os 4 canais do grupo, e só eles: um subconjunto diferente aqui exige revisitar a análise do
    /// cabeçalho do arquivo antes de mexer no teste.
    #[test]
    fn claims_exactly_the_chat_channels() {
        let claimed = modules();
        assert_eq!(claimed.len(), 1);
        assert_eq!(claimed[0].channels(), &CHANNELS);
        assert_eq!(claimed[0].name(), "chat");
    }

    /// Regressão: antes desta rodada, `modules()` fazia `Arc::new(HistoryStore::new())` a cada
    /// chamada — `features::privacy::clear::clear_history` que abrisse a MESMA função veria uma
    /// instância DIFERENTE da que `ChatModule` usa em `mika:chat-history`, esvaziando um histórico
    /// que ninguém lê. `history_store()` tem de devolver sempre o MESMO `Arc` (mesmo endereço).
    #[test]
    fn history_store_is_a_single_process_wide_instance() {
        let a = history_store();
        let b = history_store();
        assert!(Arc::ptr_eq(&a, &b), "history_store() devolveu duas instâncias diferentes de HistoryStore");
    }
}
