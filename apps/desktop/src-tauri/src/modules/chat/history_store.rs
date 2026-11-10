//! `HistoryStore` — a sessão de chat "atual" do processo Rust: mensagens em memória e, desde esta
//! rodada, também em disco (`chat_db.rs`). Ver o cabeçalho de `history.rs` para o porquê de viver
//! separada da memória episódica.
//!
//! **Por que a persistência é OPCIONAL (`Inner::db: Option<ChatDb>`).** `ChatDb::open` precisa de
//! `AppHandle` para resolver `app_data_dir`, e este crate não habilita a feature `test-support` do
//! `tauri` — nenhum teste de unidade forja um `AppHandle` real (mesma nota em `send.rs`).
//! `HistoryStore::new()` continua puramente em memória (o que os testes de `history.rs` já usavam
//! antes desta rodada); `ensure_persistence(app)` liga o disco na primeira chamada real de
//! `mika:chat-*` (`mod.rs::invoke`). O núcleo testável — anexar um `ChatDb` já pronto e restaurar
//! dele — é `attach`, exposto a teste via `attach_for_test`.
//!
//! **Por que o `seq` vem do banco a cada `push`.** `chat_write` grava na MESMA sessão por outra
//! conexão; um contador em memória ficava velho e o próximo envio batia em `UNIQUE(session_id, seq)`.
//! `push`/`clear` seguem sob um só lock para a memória e o disco andarem juntos.
//!
//! **Por que `clear()` propaga erro em vez de engolir.** Um "não consegui apagar isso do banco"
//! silencioso deixa a mensagem NA BASE enquanto a UI já mostra a conversa vazia — a classe de
//! defeito que este repo já documentou como "resposta que significa não fiz". O `?` sobe o erro
//! até o IPC; o renderer vê a falha em vez de um sucesso fabricado.

use std::sync::{Mutex, MutexGuard};

use rig_core::completion::Message as RigMessage;
use tauri::AppHandle;

use super::chat_db::ChatDb;
use super::history::ChatMessage;

struct Inner {
    messages: Vec<ChatMessage>,
    db: Option<ChatDb>,
    session_id: Option<String>,
    // 04-memory.md §"Curto prazo", MUST: resumo rolante do excedente acima de `historyLimit`,
    // gerado por LLM e guardado — nunca recalculado a cada turno. `summarized_through` é o índice
    // (0-based, exclusivo) das mensagens CONVERSACIONAIS (user/assistant, as mesmas que
    // `to_turn_history` entrega ao modelo) já cobertas pelo resumo atual — nunca de `messages` cru,
    // onde `system` (turno que falhou) deslocaria o recorte.
    rolling_summary: Option<String>,
    summarized_through: usize,
    // Geração da conversa: sobe a cada `clear`. O resumo é calculado em segundo plano; quem o pediu
    // devolve a geração que viu e uma escrita de geração velha é descartada (descrevia mensagens
    // que o usuário já apagou).
    epoch: u64,
}

/// A sessão implícita — em memória sempre, em disco desde que `ensure_persistence`/`attach` rode
/// ao menos uma vez. Ver o cabeçalho do arquivo.
pub struct HistoryStore {
    inner: Mutex<Inner>,
}

impl HistoryStore {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner { messages: Vec::new(), db: None, session_id: None, rolling_summary: None, summarized_through: 0, epoch: 0 }),
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, String> {
        self.inner.lock().map_err(|_| "histórico de chat envenenado".to_string())
    }

    /// Abre (ou reabre, sem custo depois da primeira vez) `chat.db` e restaura a sessão "atual"
    /// para a memória — a correção da regressão registrada no cabeçalho de
    /// `scripts/checks/sidecar-attrition.mts`. Chamado ANTES do gate de superfície em
    /// `mod.rs::invoke`, de propósito: abrir o arquivo não vaza dado nenhum, e assim nenhum handler
    /// individual (`history::handle_chat_history`/`handle_chat_clear`) precisa de `AppHandle` para
    /// continuar testável sem ele.
    pub fn ensure_persistence(&self, app: &AppHandle) -> Result<(), String> {
        {
            let inner = self.lock()?;
            if inner.db.is_some() {
                return Ok(());
            }
        }
        let path = super::chat_db::db_path(app)?;
        let db = ChatDb::open(&path)?;
        // `privacy.conversationRetention` poda o disco ANTES de restaurar — ver `retention.rs`. Falha
        // na poda (banco ocupado por `chat_write`) é logada e tentada de novo no próximo boot: derrubar
        // o `attach` aqui deixaria a sessão inteira sem restaurar nem gravar.
        match crate::state::config_store(app).and_then(|store| super::retention::apply(&store, &db, now_ms())) {
            Ok(0) => {}
            Ok(pruned) => log::info!("chat: {pruned} mensagem(ns) além de privacy.conversationRetention apagada(s)"),
            Err(error) => log::warn!("chat: poda de privacy.conversationRetention falhou ({error})"),
        }
        self.attach(db)
    }

    fn attach(&self, db: ChatDb) -> Result<(), String> {
        // Abertura e restauração acontecem FORA do lock (são I/O de disco); o único dado que uma
        // corrida decide é "qual `ChatDb` fica anexado", resolvido abaixo sob lock.
        let session_id = db.current_session_id()?;
        let (messages, _) = db.restore(&session_id)?;
        let rolling = db.rolling_summary(&session_id)?;

        let mut inner = self.lock()?;
        if inner.db.is_some() {
            // Outra chamada venceu a corrida de `ensure_persistence` primeiro. Mesma sessão
            // (`current_session_id` é idempotente entre conexões), então descartar esta segunda
            // abertura não perde nada — só um pouco de I/O redundante no boot.
            return Ok(());
        }
        inner.messages = messages;
        inner.session_id = Some(session_id);
        if let Some((summary, summarized_through)) = rolling {
            inner.rolling_summary = Some(summary);
            inner.summarized_through = summarized_through;
        }
        inner.db = Some(db);
        Ok(())
    }

    /// Núcleo de `ensure_persistence` sem precisar de `AppHandle` — o que os testes usam.
    #[cfg(test)]
    pub(crate) fn attach_for_test(&self, db: ChatDb) -> Result<(), String> {
        self.attach(db)
    }

    pub fn push(&self, message: ChatMessage) -> Result<(), String> {
        let mut inner = self.lock()?;
        if inner.db.is_some() {
            let session_id = inner.session_id.clone().unwrap_or_default();
            super::chat_write::append_with_retry(inner.db.as_ref().expect("checado acima"), &session_id, &message)?;
        }
        inner.messages.push(message);
        Ok(())
    }

    /// Espelha na memória uma mensagem que OUTRO escritor (`chat::append_message`, conexão própria:
    /// fala proativa, turnos do debate) já gravou no disco da sessão atual.
    ///
    /// Só entra com o disco anexado e a mesma sessão: sem disco a restauração futura lerá o arquivo
    /// inteiro (espelhar duplicaria a linha), e outra sessão não é esta conversa. Lock envenenado
    /// não derruba quem já gravou: a linha está no disco e volta no próximo boot.
    pub(crate) fn mirror_external(&self, session_id: &str, message: ChatMessage) {
        let Ok(mut inner) = self.lock() else { return };
        if inner.db.is_some() && inner.session_id.as_deref() == Some(session_id) {
            inner.messages.push(message);
        }
    }

    pub fn all(&self) -> Result<Vec<ChatMessage>, String> {
        Ok(self.lock()?.messages.clone())
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut inner = self.lock()?;
        if inner.db.is_some() {
            let session_id = inner.session_id.clone().unwrap_or_default();
            // Propaga o erro do disco — ver o porquê no cabeçalho do arquivo. Apaga também o resumo.
            inner.db.as_ref().expect("checado acima").clear_session(&session_id)?;
        }
        inner.messages.clear();
        inner.rolling_summary = None;
        inner.summarized_through = 0;
        inner.epoch += 1;
        Ok(())
    }

    /// "Apagar tudo": a sessão corrente (como [`Self::clear`]) E as demais sessões gravadas em
    /// `chat.db`. `clear()` apaga só a corrente — para o "apagar meus dados" isso deixava no disco o
    /// texto das conversas das outras sessões (quem migrou da versão TS tem várias).
    pub fn clear_all(&self) -> Result<(), String> {
        self.clear()?;
        let inner = self.lock()?;
        if let Some(db) = inner.db.as_ref() {
            db.clear_all()?;
        }
        Ok(())
    }

    /// Turnos ANTERIORES no formato que `turn::run_turn_ollama` espera. Só `user`/`assistant` (ver
    /// `ChatMessage::to_rig`); `tool` não tem representação própria porque resultado de tool só
    /// existe DENTRO do turno em curso.
    ///
    /// `limit` = `llm.historyLimit` (04-memory.md §"Curto prazo", MUST: "a janela recente entra
    /// crua no prompt, limitada por `historyLimit`") — cabe ao chamador (`send.rs`, dono do
    /// `ConfigStore` no ponto de chamada) resolver o valor configurado; aqui só corta os MAIS
    /// RECENTES `limit` turnos, nunca os mais antigos por engano (`saturating_sub` para não estourar
    /// quando `limit` excede o total). O resumo rolante do excedente (a outra MUST daquela seção)
    /// continua sem produtor em Rust — cortar sem resumir ainda perde o começo da conversa em
    /// silêncio; ver o cabeçalho de `mod.rs`.
    pub fn to_turn_history(&self, limit: usize) -> Result<Vec<RigMessage>, String> {
        let inner = self.lock()?;
        let all: Vec<RigMessage> = inner.messages.iter().filter_map(ChatMessage::to_rig).collect();
        let skip = all.len().saturating_sub(limit);
        let mut history: Vec<RigMessage> = Vec::new();
        // O resumo rolante entra ANTES da janela recente crua, como mensagem `system` — contexto
        // de memória, não algo que o usuário ou a assistente disseram de fato.
        if let Some(summary) = &inner.rolling_summary {
            history.push(RigMessage::system(format!("Automatic summary of the conversation before this window: {summary}")));
        }
        history.extend(all.into_iter().skip(skip));
        Ok(history)
    }

    /// Quanto do histórico ainda não entrou no resumo rolante, no formato que
    /// `rolling_summary::summarize_overflow_with_llm` precisa — `None` quando não há excedente novo
    /// (nada além de `historyLimit` desde o último resumo). `limit` é o MESMO `historyLimit`
    /// resolvido pelo chamador (`send.rs`), para o "excedente" ser exatamente o que
    /// `to_turn_history` já corta fora da janela recente crua — por isso os índices contam só as
    /// mensagens user/assistant, o mesmo vetor filtrado de `to_turn_history`. O terceiro valor é a
    /// geração a devolver em [`Self::set_rolling_summary`].
    pub fn overflow_for_summary(&self, limit: usize) -> Result<Option<(usize, Vec<ChatMessage>, u64)>, String> {
        let inner = self.lock()?;
        let conversation: Vec<&ChatMessage> = inner.messages.iter().filter(|m| m.to_rig().is_some()).collect();
        let overflow_end = conversation.len().saturating_sub(limit);
        if overflow_end <= inner.summarized_through {
            return Ok(None);
        }
        // Um lote por vez, das mais antigas: o prompt só carrega `MAX_MESSAGES`, e o que ele cortasse
        // seria dado por resumido (o índice avança até o fim) sem nunca ter entrado em resumo algum.
        let overflow_end = overflow_end.min(inner.summarized_through + super::rolling_summary::MAX_MESSAGES);
        let slice = conversation[inner.summarized_through..overflow_end].iter().map(|m| (*m).clone()).collect();
        Ok(Some((overflow_end, slice, inner.epoch)))
    }

    pub fn rolling_summary(&self) -> Result<Option<String>, String> {
        Ok(self.lock()?.rolling_summary.clone())
    }

    /// Substitui o resumo rolante — persiste no disco quando há `ChatDb` anexado (mesma regra de
    /// `push`/`clear`: sem sessão persistida, só o estado em memória do processo muda). `epoch` é a
    /// geração que [`Self::overflow_for_summary`] devolveu: se a conversa foi limpa desde então a
    /// escrita é obsoleta e é descartada (`Ok(false)`).
    pub fn set_rolling_summary(&self, summary: &str, summarized_through: usize, epoch: u64) -> Result<bool, String> {
        let mut inner = self.lock()?;
        if inner.epoch != epoch {
            return Ok(false);
        }
        if let (Some(db), Some(session_id)) = (&inner.db, &inner.session_id) {
            db.set_rolling_summary(session_id, summary, summarized_through, now_ms())?;
        }
        inner.rolling_summary = Some(summary.to_string());
        inner.summarized_through = summarized_through;
        Ok(true)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Default for HistoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-chat-history-test-{}-{unique}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn message(role: &str, content: &str) -> ChatMessage {
        ChatMessage { id: format!("{role}-{content}"), role: role.to_string(), content: content.to_string(), timestamp_ms: 1 }
    }

    /// A geração atual — o que um chamador que não passou por `overflow_for_summary` devolveria.
    fn epoch(store: &HistoryStore) -> u64 {
        store.inner.lock().unwrap().epoch
    }

    // -- comportamento em memória (sem attach), inalterado desde antes desta rodada -----------------

    /// `chat_write` (colaboração, turnos proativos) grava na MESMA sessão por outra conexão; o
    /// `next_seq` em memória daqui ficava velho e o próximo envio do chat batia em
    #[test]
    fn clear_all_also_erases_the_other_sessions_on_disk() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        let store = HistoryStore::new();
        store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
        store.push(message("user", "da sessão atual")).unwrap();
        ChatDb::open(&path).unwrap().append("sessao-antiga", 0, &message("user", "de uma sessão antiga")).unwrap();

        store.clear_all().unwrap();

        let reopened = ChatDb::open(&path).unwrap();
        assert!(reopened.restore("sessao-antiga").unwrap().0.is_empty(), "a sessão antiga continuava no disco");
        assert!(store.all().unwrap().is_empty());
    }

    /// `UNIQUE(session_id, seq)` — o usuário via erro ao mandar a segunda mensagem.
    #[test]
    fn push_survives_another_writer_taking_the_next_seq() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        let store = HistoryStore::new();
        store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
        store.push(message("user", "primeira")).unwrap();

        let other = ChatDb::open(&path).unwrap();
        let session_id = other.current_session_id().unwrap();
        super::super::chat_write::append_message(&other, &session_id, "assistant", "de outro escritor").unwrap();

        store.push(message("user", "segunda")).expect("segundo envio não pode colidir no seq");
        assert_eq!(other.restore(&session_id).unwrap().0.len(), 3);
    }

    /// `chat::append_message` (fala proativa, turnos do debate) grava por uma conexão própria e só no
    /// disco: a memória do processo nunca via a linha — `mika:chat-history` não a devolvia e o modelo
    /// não a tinha no contexto do turno seguinte até o app reiniciar (e aí `summarized_through`, que
    /// indexa a memória, passava a apontar para mensagens deslocadas).
    #[test]
    fn a_message_written_by_another_connection_is_mirrored_into_the_live_history() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        let store = HistoryStore::new();
        store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
        store.push(message("user", "oi")).unwrap();

        let other = ChatDb::open(&path).unwrap();
        let session_id = other.current_session_id().unwrap();
        let written = super::super::chat_write::append_message(&other, &session_id, "assistant", "fala proativa").unwrap();
        store.mirror_external(&session_id, written);

        let contents: Vec<String> = store.all().unwrap().into_iter().map(|m| m.content).collect();
        assert_eq!(contents, ["oi", "fala proativa"]);
        assert_eq!(store.to_turn_history(usize::MAX).unwrap().len(), 2);

        // Outra sessão não é a "atual" deste store: não entra na conversa.
        store.mirror_external("outra-sessao", message("assistant", "de outra sessão"));
        assert_eq!(store.all().unwrap().len(), 2);
    }

    /// Sem disco anexado a memória é a única cópia e a restauração futura lerá o arquivo inteiro:
    /// espelhar aqui duplicaria a linha no próximo boot.
    #[test]
    fn mirroring_without_an_attached_db_is_a_no_op() {
        let store = HistoryStore::new();
        store.mirror_external("qualquer", message("assistant", "x"));
        assert!(store.all().unwrap().is_empty());
    }

    /// Mais de `MAX_MESSAGES` de excedente (resumo desligado por um tempo, provedor fora, `historyLimit`
    /// reduzido) era entregue inteiro e o prompt cortava as MAIS ANTIGAS — mas `summarized_through`
    /// avançava até o fim, então essas nunca mais entravam em resumo nenhum.
    #[test]
    fn overflow_for_summary_never_hands_over_more_than_one_prompt_can_carry() {
        let max = super::super::rolling_summary::MAX_MESSAGES;
        let store = HistoryStore::new();
        for i in 0..(max * 3) {
            store.push(message("user", &format!("msg{i}"))).unwrap();
        }
        let (through, slice, epoch) = store.overflow_for_summary(10).unwrap().expect("deveria ter excedente");
        assert_eq!((through, slice.len()), (max, max));
        assert_eq!(slice[0].content, "msg0", "começa pelas mais antigas ainda sem resumo");

        assert!(store.set_rolling_summary("lote 1", through, epoch).unwrap());
        let (through2, slice2, _) = store.overflow_for_summary(10).unwrap().expect("o resto continua pendente");
        assert_eq!(slice2[0].content, format!("msg{max}"));
        assert_eq!(through2, max * 2);
    }

    #[test]
    fn starts_empty() {
        let store = HistoryStore::new();
        assert!(store.all().unwrap().is_empty());
    }

    #[test]
    fn push_then_all_returns_in_order() {
        let store = HistoryStore::new();
        store.push(message("user", "oi")).unwrap();
        store.push(message("assistant", "tudo bem?")).unwrap();
        let all = store.all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].content, "oi");
        assert_eq!(all[1].content, "tudo bem?");
    }

    #[test]
    fn clear_empties_the_store_without_a_db_attached() {
        let store = HistoryStore::new();
        store.push(message("user", "oi")).unwrap();
        store.clear().unwrap();
        assert!(store.all().unwrap().is_empty());
    }

    #[test]
    fn to_turn_history_keeps_only_user_and_assistant_in_order() {
        let store = HistoryStore::new();
        store.push(message("user", "primeira pergunta")).unwrap();
        store.push(message("assistant", "primeira resposta")).unwrap();
        store.push(message("system", "erro de um turno anterior")).unwrap();
        store.push(message("user", "segunda pergunta")).unwrap();

        let history = store.to_turn_history(usize::MAX).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0], RigMessage::user("primeira pergunta"));
        assert_eq!(history[1], RigMessage::assistant("primeira resposta"));
        assert_eq!(history[2], RigMessage::user("segunda pergunta"));
    }

    /// = 04-memory.md §"Curto prazo", MUST: "a janela recente entra crua no prompt, limitada por
    /// `historyLimit`" — sem este teto a janela crescia sem parar (ver o cabeçalho de
    /// `to_turn_history`). O corte tem de manter os MAIS RECENTES, nunca os mais antigos.
    #[test]
    fn to_turn_history_keeps_only_the_most_recent_turns_within_the_limit() {
        let store = HistoryStore::new();
        store.push(message("user", "pergunta antiga")).unwrap();
        store.push(message("assistant", "resposta antiga")).unwrap();
        store.push(message("user", "pergunta recente")).unwrap();
        store.push(message("assistant", "resposta recente")).unwrap();

        let history = store.to_turn_history(2).unwrap();
        assert_eq!(history, vec![RigMessage::user("pergunta recente"), RigMessage::assistant("resposta recente")]);
    }

    /// `limit` maior que o total nunca deve panicar (`saturating_sub`) nem truncar o que existe.
    #[test]
    fn to_turn_history_limit_above_the_total_returns_everything() {
        let store = HistoryStore::new();
        store.push(message("user", "única pergunta")).unwrap();

        let history = store.to_turn_history(20).unwrap();
        assert_eq!(history.len(), 1);
    }

    // -- persistência (attach direto num ChatDb, sem AppHandle) -------------------------------------

    #[test]
    fn push_after_attach_writes_through_and_restart_preserves_it() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");

        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            store.push(message("user", "sobrevive ao restart")).unwrap();
        }
        // "Restart": um `HistoryStore` NOVO, reaberto sobre o MESMO arquivo.
        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            let all = store.all().unwrap();
            assert_eq!(all.len(), 1);
            assert_eq!(all[0].content, "sobrevive ao restart");
        }
    }

    #[test]
    fn clear_deletes_from_disk_and_restart_confirms_it_stays_gone() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");

        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            store.push(message("user", "apagar")).unwrap();
            store.clear().unwrap();
        }
        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            assert!(store.all().unwrap().is_empty());
        }
    }

    // -- resumo rolante (04-memory.md §"Curto prazo") ------------------------------------------------

    /// MUST: "acima do limite, o excedente é resumido, não truncado" — `to_turn_history` continua
    /// cortando a janela recente crua, mas o resumo entra ANTES dela, não é descartado.
    #[test]
    fn to_turn_history_prepends_the_rolling_summary_when_present() {
        let store = HistoryStore::new();
        store.push(message("user", "pergunta antiga")).unwrap();
        store.push(message("assistant", "resposta antiga")).unwrap();
        store.set_rolling_summary("Falaram sobre RPGs.", 2, epoch(&store)).unwrap();

        let history = store.to_turn_history(usize::MAX).unwrap();
        assert_eq!(history.len(), 3);
        match &history[0] {
            RigMessage::System { content } => assert!(content.contains("Falaram sobre RPGs."), "{content}"),
            other => panic!("esperava mensagem system com o resumo, veio {other:?}"),
        }
    }

    #[test]
    fn to_turn_history_has_no_summary_message_without_one_set() {
        let store = HistoryStore::new();
        store.push(message("user", "oi")).unwrap();
        assert_eq!(store.to_turn_history(usize::MAX).unwrap().len(), 1);
    }

    /// MUST: "o resumo rolante é gerado por LLM e guardado, não recalculado a cada turno" —
    /// `overflow_for_summary` só aponta trabalho novo quando o excedente cresceu além do que já foi
    /// resumido.
    #[test]
    fn overflow_for_summary_is_none_within_the_limit() {
        let store = HistoryStore::new();
        store.push(message("user", "só uma")).unwrap();
        assert!(store.overflow_for_summary(5).unwrap().is_none());
    }

    #[test]
    fn overflow_for_summary_returns_only_the_slice_not_yet_summarized() {
        let store = HistoryStore::new();
        for i in 0..6 {
            store.push(message("user", &format!("msg{i}"))).unwrap();
        }
        // limit = 2: as 4 mais antigas transbordam.
        let (through, slice, epoch) = store.overflow_for_summary(2).unwrap().expect("deveria ter excedente");
        assert_eq!(through, 4);
        assert_eq!(slice.len(), 4);

        assert!(store.set_rolling_summary("resumo das 4 primeiras", through, epoch).unwrap());
        // Sem mensagem nova, nada além do já resumido transbordou.
        assert!(store.overflow_for_summary(2).unwrap().is_none());

        store.push(message("user", "msg6")).unwrap();
        store.push(message("user", "msg7")).unwrap();
        let (through2, slice2, _) = store.overflow_for_summary(2).unwrap().expect("nova mensagem deveria transbordar");
        assert_eq!(through2, 6);
        assert_eq!(slice2.len(), 2);
    }

    #[test]
    fn rolling_summary_survives_restart_when_persisted() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            store.push(message("user", "a")).unwrap();
            store.set_rolling_summary("resumo persistido", 1, epoch(&store)).unwrap();
        }
        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            assert_eq!(store.rolling_summary().unwrap().as_deref(), Some("resumo persistido"));
        }
    }

    /// Limpar a conversa tem de limpar o resumo dela: o resumo descreve mensagens que o usuário
    /// mandou apagar e voltava ao prompt (também depois de reiniciar) como contexto "fantasma".
    #[test]
    fn clear_drops_the_rolling_summary_from_memory_and_from_disk() {
        let dir = TempDir::new();
        let path = dir.0.join("chat.db");
        {
            let store = HistoryStore::new();
            store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
            store.push(message("user", "segredo")).unwrap();
            store.set_rolling_summary("resumo do segredo", 1, epoch(&store)).unwrap();

            store.clear().unwrap();

            assert_eq!(store.rolling_summary().unwrap(), None);
            assert!(store.to_turn_history(usize::MAX).unwrap().is_empty(), "o resumo não pode voltar ao prompt");
        }
        let store = HistoryStore::new();
        store.attach_for_test(ChatDb::open(&path).unwrap()).unwrap();
        assert_eq!(store.rolling_summary().unwrap(), None, "o resumo não pode sobreviver ao restart");
    }

    /// Corrida real: o resumo é gerado em segundo plano (`send.rs`) e o `clear` pode acontecer
    /// enquanto o LLM trabalha — a escrita que chega depois descreve uma conversa que não existe mais.
    #[test]
    fn a_summary_computed_before_a_clear_is_discarded_when_it_lands_after() {
        let store = HistoryStore::new();
        for i in 0..6 {
            store.push(message("user", &format!("msg{i}"))).unwrap();
        }
        let (through, _, epoch) = store.overflow_for_summary(2).unwrap().expect("deveria ter excedente");

        store.clear().unwrap();
        let stored = store.set_rolling_summary("resumo obsoleto", through, epoch).unwrap();

        assert!(!stored, "a escrita obsoleta tem de ser reportada como descartada");
        assert_eq!(store.rolling_summary().unwrap(), None);
    }

    /// `system` (turno que falhou) não entra no prompt nem no resumo: os índices de
    /// `overflow_for_summary`/`summarized_through` têm de contar só user/assistant, como
    /// `to_turn_history` — contar a lista crua deslocava o recorte e resumia a mensagem errada.
    #[test]
    fn overflow_for_summary_indexes_only_user_and_assistant_messages() {
        let store = HistoryStore::new();
        store.push(message("user", "a")).unwrap();
        store.push(message("system", "erro")).unwrap();
        store.push(message("assistant", "b")).unwrap();
        store.push(message("system", "erro")).unwrap();
        store.push(message("user", "c")).unwrap();
        store.push(message("assistant", "d")).unwrap();

        let (through, slice, epoch) = store.overflow_for_summary(2).unwrap().expect("a e b transbordam");
        assert_eq!(through, 2);
        assert_eq!(slice.iter().map(|m| m.content.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);

        store.set_rolling_summary("resumo de a e b", through, epoch).unwrap();
        assert!(store.overflow_for_summary(2).unwrap().is_none(), "nada novo além do já resumido");
    }

    #[test]
    fn concurrent_pushes_never_collide_on_seq() {
        let store = Arc::new(HistoryStore::new());
        store.attach_for_test(ChatDb::open_in_memory().unwrap()).unwrap();

        let handles: Vec<_> = (0..8)
            .map(|i| {
                let store = Arc::clone(&store);
                std::thread::spawn(move || store.push(message("user", &format!("msg-{i}"))).unwrap())
            })
            .collect();
        // Se dois `push` tivessem calculado o mesmo `seq`, o índice único de `chat_db.rs` teria
        // rejeitado a segunda escrita e o `.unwrap()` dentro da thread teria entrado em pânico —
        // `join()` propaga esse pânico para este teste.
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(store.all().unwrap().len(), 8);
    }
}
