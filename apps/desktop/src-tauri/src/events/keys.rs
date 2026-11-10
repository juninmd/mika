//! `EventKey` — o vocabulário fechado de nomes válidos no barramento (`crate::events::bus`).
//! Espelha `Events` de `packages/sdk/src/svc/events.ts`, mas NÃO é uma cópia integral: aquele
//! arquivo declara ~56 chaves, esta enum só as que um módulo Rust já portado ou EM PORTE nesta
//! rodada precisa publicar ou assinar. As outras ficam de fora de propósito — adicioná-las sem um
//! chamador real seria enum morto disfarçado de pronto, o mesmo critério que `modules/browser.rs` e
//! `modules/features/mood/mod.rs` já aplicaram para os próprios achados (nenhum dos dois reivindicou
//! canal ou evento sem alguém para usar). Crescer esta lista é esperado; crescê-la especulativamente
//! não é.
//!
//! ## Por que estas variantes entraram
//!
//! - **`SpeechStarted` / `SpeechFinished` / `ForceStopTts` / `Interruption`** — o que faltava para o
//!   barge-in sair do papel. `modules/voice/interruption_detector.rs` já tem `on_speech_started` /
//!   `on_speech_finished`, e `modules/voice/barge_in_recorder.rs` já tem `on_force_stop_tts` — os
//!   dois só podiam ser chamados direto em teste, nunca a partir de um evento real, porque a chave
//!   não existia.
//! - **`ListeningStarted` / `ListeningStopped` / `MuteSpeech` / `SpeechRequest` / `TtsAudioReady`**
//!   — o resto do domínio `voice` que o mesmo consumidor real do TS (`voice-barge-in.ts`) e a
//!   frente de TTS em porte nesta rodada precisam para fechar o ciclo escuta → fala → interrupção.
//!   Deixar essas de fora agora só adiaria a mesma lacuna para a próxima rodada de voz — o módulo
//!   que já existe (`modules/voice/`) é exatamente o dono natural delas. `TtsFallback` (fallback
//!   ElevenLabs->local) NÃO entrou: nunca foi portado (`voice/tts/speaker/mod.rs`,
//!   "reportado, não implementado"), então a variante ficaria declarada sem publicador nenhum —
//!   mesmo critério de "sem chamador real" do parágrafo acima, aplicado a ela mesma
//!   (`check:events`, 2026-09-22).
//! - **`MoodChanged` / `MoodAffinityLevelUp`** — dívida nomeada em `modules/features/mood/mod.rs`
//!   (seção "Três divergências do TS" do cabeçalho daquele arquivo): a tool `mood` já calcula os
//!   dois valores, só não publicava por falta desta chave.
//! - **`Expression` / `Notification`** — adicionadas numa segunda rodada, quando a frente de
//!   `features` achou os dois bloqueando trabalho concreto: a ação `expression` da tool `character`
//!   degradava por falta de `Expression`, e o disparo automático do `scheduler` (que precisa
//!   publicar `Notification` quando um lembrete vence) não tinha onde publicar. Mesma regra das
//!   demais — a chave só entra quando alguém precisa dela de verdade, e agora alguém precisa.
//! - **`AssistantMessage`** — dívida nomeada nos cabeçalhos de `brain::tools::content` e
//!   `brain::tools::collaboration`/`collaboration_session`: as duas tools já chamavam o LLM de
//!   verdade (o recap diário, cada turno de um debate multi-agente), mas o texto gerado só existia
//!   dentro do `data` de retorno da tool — nunca chegava à UI, porque esta chave não existia. Ver
//!   `modules/chat/assistant_bridge.rs` para quem encaminha isto ao IPC do renderer, e
//!   `modules/brain/tools/assistant_message.rs` para quem publica.
//! - **`PowerResume` / `PowerBattery`** — fecha a lacuna nomeada em `power.rs` (poller de energia
//!   apagado junto com o sidecar Node, sem chave nem consumidor): `modules/system_signals/power.rs`
//!   volta a publicar as duas transições reais (retomou de suspensão, desconectou da tomada) por
//!   salto de relógio de parede/borda de estado — mesma técnica que o TS antigo tirava do
//!   `powerMonitor` do Electron. `brain::awareness::event_reactions` consome as duas para as
//!   reações "sistema acordou"/"saiu da tomada" que `awareness-event-handlers.ts` já tinha.
//! - **`ActivityChanged`** — port de `ExpressionSyncService` (`services/voice/services/
//!   expression-sync.ts`): `modules/voice/expression_sync.rs` publica o estado de atividade da Mika
//!   (`idle`/`listening`/`thinking`/`speaking`/`error`) a cada transição de `ThinkingStart`/
//!   `ThinkingStop`/`SpeechStarted`/`SpeechFinished`/`ListeningStarted`/`ListeningStopped`/
//!   `ChatError`. `hud_bridge.rs` encaminha ao canal `hud:activity` que a UI já espera
//!   (`DESKTOP_BRIDGED_EVENT_CHANNELS.Activity`).
//!
//! ## Tolerância a perda, por evento
//!
//! `crate::events::bus` DESCARTA para um assinante lento (ver o cabeçalho de `events/bus.rs` para o
//! mecanismo). Não existe resposta genérica — cada variante abaixo foi avaliada olhando quem
//! consome no lado TS:
//!
//! | Variante | Tolera perda? | Por quê |
//! |---|---|---|
//! | `VadAudio` | **Não** (risco aceito, monitorado) | `barge-in-recorder.ts` grava a partir da janela bruta; perder uma no meio corrompe o trecho. |
//! | `VadSample` | Sim | TS já throttla a 100 Hz antes de emitir; perder uma amostra é a mesma degradação que o throttle já aceita. |
//! | `SpeechStarted` / `SpeechFinished` | **Não** (risco aceito, baixa frequência) | São a transição de estado que `InterruptionDetector` usa para saber se a Mika está falando. Perder uma trava o detector no estado errado até o próximo par — mas o par só ocorre uma vez por fala da Mika (não por amostra), então a chance de cair numa janela de lag do anel de 256 é muito menor que a de `VadAudio`. |
//! | `ForceStopTts` | **Não** (risco aceito, baixa frequência) | É a ATUAÇÃO do barge-in: se `barge_in_recorder.rs::on_force_stop_tts` nunca for chamado, a Mika continua falando por cima do usuário naquele turno. Mesma lógica de frequência baixa de `SpeechStarted`: dispara só depois do streak de interrupção confirmado, não por janela. |
//! | `Interruption` | **Não** (risco aceito, baixa frequência) | Mesmo consumidor e mesma atuação de `ForceStopTts` (ambos chegam juntos de `voice-barge-in.ts` no TS); perder um não derruba o TTS mas perde o motivo/transcript da interrupção para quem grava histórico. |
//! | `ListeningStarted` / `ListeningStopped` | Sim | Consumidos hoje só para telemetria de UI (ícone da bandeja, HUD) e para o auto-speak saber se pode falar por cima; perder um é o app ficar um ciclo atrasado num estado de UI, não uma perda de dado do usuário. |
//! | `MuteSpeech` | Sim | Comando "silenciar até X ms" — se perdido, o próximo `MuteSpeech` (ou o timeout `untilMs` já embutido no payload) corrige o estado; não há acumulação de efeito perdido. |
//! | `SpeechRequest` | **Não** (risco aceito, baixa frequência) | É o pedido para a Mika falar uma frase (`voice-auto-speak-events.ts` consome para disparar TTS); perder um é a Mika silenciosamente não dizer algo que devia — mesma classe de "resposta que significa não fiz" já catalogada neste repositório. Frequência baixa (por turno de fala, não por amostra) mantém o risco de lag pequeno. |
//! | `TtsAudioReady` | **Não** (risco aceito, baixa frequência) | Cada mensagem é ÁUDIO SINTETIZADO de uma frase inteira (`tts-speaker.ts` emite uma por chunk de `splitIntoSentences`, não por amostra de PCM); perder uma é a Mika pular uma frase inteira ao falar. Frequência baixa (frases por turno, não amostras por segundo) é o que torna aceitável colocar isso no mesmo anel de 256 que `VadAudio`. |
//! | `TtsFallback` | Sim | Aviso informativo (por que a voz mudou para a local); a própria voz que sai já é o sinal audível. Perder o evento não muda o áudio falado, só um toast que a UI deixaria de mostrar naquela frase. |
//! | `MoodChanged` | Sim | Estado consultável a qualquer momento via a própria tool `mood` (`action=get`); o evento é só a notificação PUSH de que mudou. Um assinante que perde uma notificação vê o valor certo na próxima consulta. |
//! | `MoodAffinityLevelUp` | Sim | Mesma razão de `MoodChanged` — é um aviso de marco (para notificação/HUD), não a fonte de verdade do nível, que continua legível via `action=get`. |
//! | `Expression` | Sim | Só pede pro avatar trocar a pose/expressão renderizada; se perdida, o avatar fica um ciclo atrasado na expressão anterior até o próximo `Expression` chegar (fala, mudança de humor, etc. disparam um novo a qualquer momento) — mesma classe de "estado sempre resincronizado pelo próximo evento" que `MoodChanged`. |
//! | `Notification` | **Não** (risco aceito, baixa frequência) | Ao contrário de `MoodChanged`, não existe um `action=get` para reconsultar — é a ÚNICA vez que aquele fato (lembrete venceu, MCP falhou, provedor degradou) é anunciado. Perder é o usuário nunca saber que algo aconteceu, mesma classe de "resposta que significa não fiz" já catalogada neste repositório. Frequência baixa (notificações são esparsas, não um stream) mantém o risco de lag pequeno, mesmo padrão de `SpeechRequest`/`ForceStopTts`. |
//! | `AssistantMessage` | **Não** (risco aceito, baixa frequência) | É o texto do recap/debate chegando à tela de chat (`chat:assistant-message`) — sem `action=get` para reconsultar depois, mesma classe de `Notification`/`SpeechRequest`. Frequência baixa: um recap por chamada da tool `content`, e um debate tem no máximo 8 turnos com 3s de pausa entre eles. |
//! | `PowerResume` / `PowerBattery` | Sim | São bordas de transição raras (poll de 15s); perder uma faz o próximo poll ainda enxergar o novo estado (bateria continua desconectada até religar), só a fala reativa daquele instante específico não sai — degradação aceitável, mesma classe de `MoodChanged`. |
//! | `HardwareAlert` | Sim | Borda de piora (poll de 30s); o toast `Notification` sai do mesmo tick e é o aviso garantido — perder este evento só cala a fala da Mika daquela vez. |
//! | `SensesPaused` | Sim | É o estado de UI do indicador de sentidos; o próximo toggle ressincroniza, e o toast `Notification` do mesmo clique é o aviso garantido. |
//! | `ActivityChanged` | Sim | Estado de UI (ícone/expressão) sempre resincronizado pela PRÓXIMA transição de atividade, que ocorre a cada troca de estado de voz — mesma razão de `Expression`. |
//!
//! As variantes marcadas "risco aceito" seguem o mesmo tratamento que `VadAudio` já tinha antes
//! desta rodada: iam para o barramento MESMO SEM garantia de entrega, porque a alternativa (não
//! declarar a chave) bloqueava o consumidor real por completo — mas a perda é `log::warn!`ada e
//! contabilizada (`crate::events::lagged_total`), nunca silenciosa. Nenhuma delas tem frequência de
//! disparo perto da de `VadAudio`/`VadSample` (a única de fato contínua), o que mantém a
//! probabilidade de cair numa janela de lag baixa mesmo sem essa garantia formal.

/// Chave tipada de evento — ver o cabeçalho deste arquivo para o critério de quando uma variante
/// nova entra e a tabela de tolerância a perda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKey {
    /// Janela de áudio PCM16 bruta do VAD. NÃO tolera perda — ver a tabela do cabeçalho.
    VadAudio,
    /// Probabilidade de fala por janela do VAD, já throttlada no lado TS. Tolera perda.
    VadSample,

    /// A Mika começou a falar (TTS). Consumido por `InterruptionDetector::on_speech_started`.
    SpeechStarted,
    /// A Mika terminou de falar (TTS) — emitido sempre, mesmo em erro/abort (bloco `finally` do
    /// lado TS). Consumido por `InterruptionDetector::on_speech_finished`.
    SpeechFinished,
    /// Um chunk de áudio sintetizado (uma frase) está pronto para tocar.
    TtsAudioReady,
    /// ElevenLabs caiu para a voz local (Supertonic) — sem credencial, sem crédito ou voz não
    /// coberta pelo plano. Reintroduzida nesta rodada: `voice/tts/dispatch.rs` agora é o publicador
    /// real (ver o histórico desta variante no cabeçalho de `packages/sdk/src/svc/events.ts`).
    TtsFallback,
    /// O microfone começou a escutar.
    ListeningStarted,
    /// O microfone parou de escutar.
    ListeningStopped,
    /// Pedido para a Mika falar um texto (origem: personagem, comentário de jogo, etc.).
    SpeechRequest,
    /// Comando para silenciar a fala até um instante (`untilMs` no payload).
    MuteSpeech,

    /// Sinal de atuação do barge-in: para o TTS em andamento agora.
    ForceStopTts,
    /// O usuário interrompeu a Mika falando por cima — carrega o motivo/streak da detecção.
    Interruption,

    /// O estado de humor (valência/excitação/rótulo) mudou.
    MoodChanged,
    /// A pontuação de afinidade cruzou um novo nível.
    MoodAffinityLevelUp,

    /// O avatar deve trocar de expressão/pose visual.
    Expression,
    /// Aviso pontual para o usuário (lembrete, menção, falha de provedor, etc.) — sem `action=get`
    /// para reconsultar depois; ver a tabela de tolerância a perda no cabeçalho.
    Notification,

    /// Texto gerado por LLM fora do turno principal de chat (recap diário, turno de debate) pronto
    /// para aparecer na tela — mesmo papel de `Events.ASSISTANT_MESSAGE` no TS.
    AssistantMessage,
    /// O turno de chat começou (`chat::send` antes do primeiro token) — a fala automática abre a
    /// sessão de frases e o motor proativo marca `userTurnInFlight`.
    ThinkingStart,
    /// O turno de chat terminou (sucesso, erro ou cancelamento) — descarrega o resto da fala.
    ThinkingStop,
    /// Um pedaço de texto do stream do turno — a fala automática recorta frases completas dele.
    StreamChunk,
    /// O modelo chamou uma tool no meio do turno — a fala descarrega o que já tem antes da espera.
    ToolCall,
    /// A visão descreveu a tela (observador periódico ou `take_screenshot`) — a percepção do motor
    /// proativo e o estado de comportamento reagem ao que a Mika viu.
    ScreenObservation,
    /// Uma imagem nova apareceu na área de transferência, com consentimento — a visão a analisa.
    VisionClipboardImage,
    /// O turno de chat falhou de verdade (provedor fora, erro de modelo) — abort não conta. A
    /// bandeja mostra o estado de erro, como `Events.ERROR` fazia no Electron.
    ChatError,
    /// Uma tool do turno terminou — o catálogo de pessoas não re-extrai o que a tool acabou de
    /// gravar (janela de carência do `person-catalog-mention-gate.ts`).
    ToolResult,
    /// Um jogo passou a rodar em primeiro plano (`game_watcher`) — reação proativa e contexto de
    /// jogo no prompt.
    GameChanged,
    /// O jogo que rodava fechou.
    GameStopped,
    /// O tema claro/escuro do SO mudou (só chega como evento de janela, ver `windows.rs`).
    ThemeChanged,

    /// O sistema retomou de suspensão (poller de energia, ver `modules/system_signals`).
    PowerResume,
    /// O sistema desconectou da tomada (transição para bateria).
    PowerBattery,

    /// A atividade da Mika mudou (`idle`/`listening`/`thinking`/`speaking`/`error`) — port de
    /// `Events.ACTIVITY_CHANGED` (`ExpressionSyncService`).
    ActivityChanged,

    /// A janela em primeiro plano do SO mudou (dono/título) — port de
    /// `Events.OS_ACTIVE_WINDOW_CHANGED`. Publicador: `modules/system_signals/active_window.rs`
    /// (Windows apenas, mesmo corte do TS). Consumidores: reação proativa "window change"
    /// (`brain::awareness::event_reactions::on_window_changed`) e o modo `technical` de
    /// `features::character::slang_context` (leitura pull-based, não assinante).
    ActiveWindowChanged,
    /// Um pensamento pronto para a Mika dizer por iniciativa própria (análise de imagem do
    /// clipboard, atalho de screenshot) — `ThoughtService` decide se e quando vira fala.
    BrainProactiveThought,

    /// Um momento notável de jogo (boss, cutscene, vitória, derrota) — port de `Events.GAME_MOMENT`.
    /// Publicador: `modules/gamer/rpc.rs::MiniGameResult` (vitória/derrota num mini-jogo). No TS
    /// existiam 4 consumidores (`game-moment-commentary.ts`, `slang-context-provider.ts`,
    /// `memory/experience.ts`, `vision/processing/observation-publisher.ts`); aqui só o relógio de
    /// tensão entra nesta rodada (`brain::awareness::tension`, consumido pelo gate de
    /// `ProactiveEngine::should_speak`) — os outros 3 seguem sem porte, nomeados, não escondidos.
    GameMoment,

    /// O sistema reconectou à tomada — a borda simétrica de [`Self::PowerBattery`]
    /// (`false` -> `true` de `on_battery_power` vira `true` -> `false`). Publicador:
    /// `modules/system_signals/power.rs`, mesmo poller, mesma técnica de borda de estado.
    PowerAc,

    /// Uma pessoa do catálogo mudou (upsert que não é a primeira vez, ou o patch de
    /// `update_social`) — porte de `Events.PERSON_UPDATED`. A UI de Pessoas ouve isto para
    /// recarregar a lista sem polling. Publicador: `person_catalog::tool.rs::handle_upsert`/
    /// `handle_update_social` e `lifecycle::upsert_extracted_person` (extração automática do chat).
    PersonUpdated,
    /// Uma pessoa foi apagada do catálogo — porte de `Events.PERSON_DELETED`. Publicador:
    /// `person_catalog::tool.rs::handle_delete`.
    PersonDeleted,
    /// Uma nova menção foi registrada numa pessoa já catalogada — porte de
    /// `Events.PERSON_MENTION_ADDED`. O renderer trata isto como o MESMO sinal de "recarregue a
    /// lista" que [`Self::PersonUpdated`] (`mika-event-channels.ts` mapeia os dois ao mesmo canal
    /// `hud:person-catalog-updated`), então este módulo publica os dois na mesma chave de canal,
    /// nunca inventa uma segunda. Publicador: `person_catalog::tool.rs::handle_add_mention`.
    PersonMentionAdded,

    /// A folga de memória do PC piorou de nível (`warning`/`critical`) — publicador:
    /// `modules/system_signals/memory.rs`. Consumidor: reação proativa `hardware:memory`.
    HardwareAlert,

    /// "Pausar mic e tela" (bandeja) mudou — payload `{ "paused": bool }`. Publicador:
    /// `modules/privacy/senses.rs`. Consumidor: a ponte HUD (`hud:senses-paused`), que faz o
    /// `SensesMonitor` mostrar mic e visão desligados.
    SensesPaused,
}

impl EventKey {
    /// Todas as variantes — usado pelo teste de contrato para não deixar nenhuma sem checagem
    /// contra `svc/events.ts` por esquecimento.
    #[cfg(any(test, feature = "e2e"))]
    pub const ALL: &'static [EventKey] = &[
        Self::VadAudio,
        Self::VadSample,
        Self::SpeechStarted,
        Self::SpeechFinished,
        Self::TtsAudioReady,
        Self::TtsFallback,
        Self::ListeningStarted,
        Self::ListeningStopped,
        Self::SpeechRequest,
        Self::MuteSpeech,
        Self::ForceStopTts,
        Self::Interruption,
        Self::MoodChanged,
        Self::MoodAffinityLevelUp,
        Self::Expression,
        Self::Notification,
        Self::AssistantMessage,
        Self::ThinkingStart,
        Self::ThinkingStop,
        Self::StreamChunk,
        Self::ToolCall,
        Self::ScreenObservation,
        Self::VisionClipboardImage,
        Self::ChatError,
        Self::ToolResult,
        Self::GameChanged,
        Self::GameStopped,
        Self::ThemeChanged,
        Self::PowerResume,
        Self::PowerBattery,
        Self::ActivityChanged,
        Self::ActiveWindowChanged,
        Self::BrainProactiveThought,
        Self::GameMoment,
        Self::PowerAc,
        Self::PersonUpdated,
        Self::PersonDeleted,
        Self::PersonMentionAdded,
        Self::HardwareAlert,
        Self::SensesPaused,
    ];

    /// O literal exato do lado TypeScript (`packages/sdk/src/svc/events.ts`). O consumidor real
    /// assina por esse texto — o Rust nunca inventa o próprio nome.
    #[cfg(any(test, feature = "e2e"))]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VadAudio => "voice:vad-audio",
            Self::VadSample => "voice:vad-sample",
            Self::SpeechStarted => "voice:speech-started",
            Self::SpeechFinished => "voice:speech-finished",
            Self::TtsAudioReady => "voice:tts-audio-ready",
            Self::TtsFallback => "voice:tts-fallback",
            Self::ListeningStarted => "voice:listening-started",
            Self::ListeningStopped => "voice:listening-stopped",
            Self::SpeechRequest => "speech:request",
            Self::MuteSpeech => "mika:mute-speech",
            Self::ForceStopTts => "voice:force-stop-tts",
            Self::Interruption => "voice:interruption",
            Self::MoodChanged => "mood:changed",
            Self::MoodAffinityLevelUp => "mood:affinity-level-up",
            Self::Expression => "avatar:expression",
            Self::Notification => "notification",
            Self::AssistantMessage => "chat:assistant-message",
            Self::ThinkingStart => "chat:thinking-start",
            Self::ThinkingStop => "chat:thinking-stop",
            Self::StreamChunk => "chat:stream-chunk",
            Self::ToolCall => "chat:tool-call",
            Self::ScreenObservation => "vision:screen-observation",
            Self::VisionClipboardImage => "vision:clipboard-image",
            Self::ChatError => "chat:error",
            Self::ToolResult => "chat:tool-result",
            Self::GameChanged => "steam:game-changed",
            Self::GameStopped => "steam:game-stopped",
            Self::ThemeChanged => "system:theme-changed",
            Self::PowerResume => "system:power-resume",
            Self::PowerBattery => "system:power-battery",
            Self::ActivityChanged => "mika:activity",
            Self::ActiveWindowChanged => "os:active-window-changed",
            Self::BrainProactiveThought => "brain:proactive-thought",
            Self::GameMoment => "steam:game-moment",
            Self::PowerAc => "system:power-ac",
            Self::PersonUpdated => "person-catalog:updated",
            Self::PersonDeleted => "person-catalog:deleted",
            Self::PersonMentionAdded => "person-catalog:mention-added",
            Self::HardwareAlert => "hardware:alert",
            Self::SensesPaused => "privacy:senses-paused",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O ponto crítico do arquivo: um nome digitado errado aqui não quebra a compilação, só faz o
    /// publicador falar sozinho no vazio e o assinante esperar para sempre — exatamente a classe de
    /// defeito que este repositório já pagou ("gate que perde o alvo fica verde e otimista, não
    /// vermelho"). `EventKey::ALL` garante que uma variante nova não escapa desta checagem por
    /// esquecimento de adicionar ao array manual que o teste original usava.
    #[test]
    fn event_key_literals_match_the_typescript_contract() {
        let events_ts = include_str!("../../../../../packages/sdk/src/svc/events.ts");
        for key in EventKey::ALL {
            let literal = key.as_str();
            assert!(
                events_ts.contains(&format!("'{literal}'")),
                "literal '{literal}' não existe em packages/sdk/src/svc/events.ts — a chave Rust divergiu do contrato"
            );
        }
    }

    /// Nenhuma variante pode reaproveitar o literal de outra por copiar-e-colar — cada chave Rust
    /// tem de ter um nome TS distinto, senão dois eventos diferentes colidiriam no mesmo canal.
    #[test]
    fn every_variant_has_a_distinct_literal() {
        let mut literals: Vec<&str> = EventKey::ALL.iter().map(|key| key.as_str()).collect();
        let before = literals.len();
        literals.sort_unstable();
        literals.dedup();
        assert_eq!(literals.len(), before, "duas variantes de EventKey compartilham o mesmo literal TS");
    }
}
