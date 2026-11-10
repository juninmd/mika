//! Porte fiel de `apps/desktop/src/main/services/voice/audio/interruption-detector.ts` — a máquina
//! de estado que decide SE uma janela de fala do usuário conta como barge-in, não o que fazer
//! depois de decidir (isso continua fora, ver o cabeçalho de `vad_pipeline.rs`).
//!
//! **Uma diferença deliberada do original.** O TS liga a checagem de silêncio (`endpointing`) a um
//! `setInterval` próprio, porque nada mais garante que `feedVadSample` seja chamado quando a fala
//! pára. Aqui não existe esse risco: `vad_pipeline.rs` chama [`InterruptionDetector::feed_vad_sample`]
//! uma vez por JANELA do VAD, e o microfone captado continua emitindo janelas em ~31 Hz enquanto
//! estiver ligado — mesma garantia de cadência que `events.rs` já documenta para `VadSample`. Por
//! isso [`InterruptionDetector::check_silence_endpoint`] é chamado pelo pipeline a cada janela em
//! vez de por um timer solto; resolução igual ou melhor que os `max(100, silenceMs/10)` ms do TS,
//! sem precisar de uma segunda tarefa assíncrona.
//!
//! **Config, valores e nomes de campo** espelham `interruption-options.ts` byte a byte
//! (`DEFAULT_THRESHOLD`, `DEFAULT_STREAK`, `DEFAULT_COOLDOWN_MS`, `DEFAULT_SILENCE_MS`,
//! `DEFAULT_ENDPOINTING_ENABLED`, `NEURAL_SPEECH_THRESHOLD`) — as chaves de config que os alimentam
//! (`voice.interruption*`, `voice.vadEndpointingEnabled`, `voice.vadSilenceMs`) são relidas a cada
//! chamada de `voice:vad-audio` (`vad_audio.rs::VadAudioSession::refresh_settings`), que monta este
//! [`InterruptionConfig`] a partir do `ConfigStore`. [`InterruptionDetector::update_config`] zera o
//! streak em curso, então `VadPipeline::set_interruption_config` só o chama quando a config mudou.

/// Limiar de probabilidade neural do Silero acima do qual uma janela conta como fala — mesmo valor
/// de `NEURAL_SPEECH_THRESHOLD` no TS.
pub const NEURAL_SPEECH_THRESHOLD: f32 = 0.5;

/// defects.md "barge-in", item 4: logo que a reprodução começa, o AEC do sistema ainda não pegou o
/// eco — a própria voz da Mika pode cruzar `NEURAL_SPEECH_THRESHOLD` e disparar um barge-in nela
/// mesma. Nesta janela de graça, uma amostra só conta como fala com evidência mais forte
/// (`ECHO_GRACE_THRESHOLD`), nunca com um streak mais longo: um streak maior atrasaria também uma
/// interrupção REAL (o teste `a_strong_onset_still_triggers_quickly_during_the_echo_grace_window`
/// prova que ela ainda dispara dentro de ~150ms).
const ECHO_GRACE_MS: u64 = 300;
/// Acima do normal (0.5) o bastante para que eco típico (probabilidade morna, perto do limiar) não
/// passe, mas uma fala real e alta ainda cruze com folga.
const ECHO_GRACE_THRESHOLD: f32 = 0.75;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InterruptionConfig {
    pub enabled: bool,
    /// Limiar de amplitude usado só quando não há probabilidade neural (fallback de `feedVadSample`).
    pub vad_threshold: f32,
    pub trigger_streak: u32,
    pub cooldown_ms: u64,
    pub endpointing_enabled: bool,
    pub silence_ms: u64,
    /// Limiar de probabilidade neural do Silero (`voice.vadSpeechThreshold`, `config/schemas/voice.rs`)
    /// — antes desta rodada, um `NEURAL_SPEECH_THRESHOLD` fixo ignorava o controle da UI por
    /// completo (a Aba Voz salvava o valor e nada o lia). Default aqui continua sendo o valor
    /// histórico do detector (0.5), não o default do schema (0.35): `vad_audio.rs::interruption_config_from_store`
    /// é quem lê o schema de verdade; este `Default` só cobre os testes deste arquivo e o boot sem
    /// `ConfigStore` (`vad_pipeline.rs`).
    pub speech_threshold: f32,
}

impl Default for InterruptionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            vad_threshold: 0.15,
            trigger_streak: 3,
            cooldown_ms: 1_200,
            endpointing_enabled: true,
            silence_ms: 1_500,
            speech_threshold: NEURAL_SPEECH_THRESHOLD,
        }
    }
}

/// O que uma janela alimentada pode produzir — espelha os dois disparos que `interruption-transcript.ts`
/// (`INTERRUPTION_EVENT`) e `interruption-detector.ts` (`FORCE_STOP_TTS_EVENT`) emitiam no TS. Quem
/// ATUA sobre cada variante (parar TTS, transcrever, abortar o turno) fica fora deste arquivo —
/// ver `vad_pipeline.rs`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterruptionSignal {
    None,
    /// Streak de fala real durante a fala da Mika, fora do cooldown — o gatilho de `FORCE_STOP_TTS`.
    BargeIn { level: f32, at_ms: u64 },
    /// Silêncio sustentado depois de o usuário ter falado — o `is_silence_endpoint` do TS.
    SilenceEndpoint { at_ms: u64 },
}

pub struct InterruptionDetector {
    config: InterruptionConfig,
    speaking: bool,
    /// The renderer is still playing Mika's audio — outlives `speak()`, which ends at synthesis.
    playback: bool,
    /// Quando a reprodução começou (`None` = nunca, ou já reencerrada) — só usado para saber se
    /// ainda estamos dentro de `ECHO_GRACE_MS`, ver [`Self::effective_speech_threshold`].
    playback_started_at_ms: Option<u64>,
    streak: u32,
    /// `None` = nunca disparou. Um `u64` puro em 0 colidiria com `now_ms == 0` e o primeiro
    /// gatilho possível do processo entraria em cooldown consigo mesmo — bug real, pego pelo teste
    /// `triggers_barge_in_after_reaching_the_configured_streak` antes deste `Option`.
    last_interrupt_at_ms: Option<u64>,
    user_voiced_this_turn: bool,
    last_vad_activity_at_ms: u64,
}

impl InterruptionDetector {
    pub fn new(config: InterruptionConfig) -> Self {
        Self {
            config,
            speaking: false,
            playback: false,
            playback_started_at_ms: None,
            streak: 0,
            last_interrupt_at_ms: None,
            user_voiced_this_turn: false,
            last_vad_activity_at_ms: 0,
        }
    }

    /// Mesmo efeito de `updateConfig` no TS: zera o streak em curso — uma mudança de limiar no meio
    /// de uma sequência não deve contar amostras avaliadas sob a config antiga.
    ///
    /// Chamado por `VadPipeline::set_interruption_config` só quando a config mudou de verdade.
    pub fn update_config(&mut self, config: InterruptionConfig) {
        self.config = config;
        self.streak = 0;
    }

    pub fn config(&self) -> InterruptionConfig {
        self.config
    }

    /// Limiar do Silero desta sessão — `speech_gate::SpeechGate` usa o MESMO, nunca um segundo.
    pub fn speech_threshold(&self) -> f32 {
        self.config.speech_threshold
    }

    /// Mika começou a falar (equivalente a assinar `Events.SPEECH_STARTED`). Quem dispara isso
    /// ainda não existe em Rust — ver o cabeçalho de `vad_pipeline.rs`.
    pub fn on_speech_started(&mut self, now_ms: u64) {
        self.speaking = true;
        self.streak = 0;
        self.user_voiced_this_turn = false;
        self.last_vad_activity_at_ms = now_ms;
    }

    /// Edge-triggered: re-sending the same state every VAD window must not reset the streak.
    pub fn set_playback(&mut self, playing: bool, now_ms: u64) {
        if playing == self.playback {
            return;
        }
        self.playback = playing;
        if playing {
            self.playback_started_at_ms = Some(now_ms);
        }
        if playing && !self.speaking {
            self.on_speech_started(now_ms);
        } else if !playing {
            self.on_speech_finished();
        }
    }

    /// `self.config.speech_threshold` (`voice.vadSpeechThreshold`) normalmente;
    /// `ECHO_GRACE_THRESHOLD` só nos primeiros `ECHO_GRACE_MS` depois que a reprodução começou —
    /// ver o comentário de `ECHO_GRACE_MS`.
    fn effective_speech_threshold(&self, now_ms: u64) -> f32 {
        match self.playback_started_at_ms {
            Some(started) if self.playback && now_ms.saturating_sub(started) < ECHO_GRACE_MS => ECHO_GRACE_THRESHOLD,
            _ => self.config.speech_threshold,
        }
    }

    pub fn on_speech_finished(&mut self) {
        // Synthesis ended but the audio is still playing: she is still talking.
        if self.playback {
            return;
        }
        self.speaking = false;
        self.streak = 0;
        self.user_voiced_this_turn = false;
    }

    /// Porte de `feedVadSample`. `speech_probability` é o caminho real (Silero, via `vad_pipeline.rs`);
    /// `level` só importa quando não há probabilidade — mesma prioridade do TS.
    pub fn feed_vad_sample(&mut self, level: Option<f32>, speech_probability: Option<f32>, now_ms: u64) -> InterruptionSignal {
        let is_voice = match speech_probability {
            Some(p) => p >= self.effective_speech_threshold(now_ms),
            None => level.is_some_and(|l| l >= self.config.vad_threshold),
        };

        if is_voice {
            self.last_vad_activity_at_ms = now_ms;
            self.user_voiced_this_turn = true;
        }

        if !self.config.enabled || !self.speaking || !is_voice {
            self.streak = 0;
            return InterruptionSignal::None;
        }

        self.streak += 1;
        if self.streak < self.config.trigger_streak {
            return InterruptionSignal::None;
        }
        if let Some(last) = self.last_interrupt_at_ms {
            if now_ms.saturating_sub(last) < self.config.cooldown_ms {
                self.streak = 0;
                return InterruptionSignal::None;
            }
        }

        self.last_interrupt_at_ms = Some(now_ms);
        self.streak = 0;
        self.speaking = false;
        InterruptionSignal::BargeIn { level: level.unwrap_or(0.0), at_ms: now_ms }
    }

    /// Porte do corpo do `setInterval` de `startSilenceDetection`. Ver o cabeçalho do arquivo para
    /// o porquê de virar uma chamada por janela em vez de um timer.
    pub fn check_silence_endpoint(&mut self, now_ms: u64) -> InterruptionSignal {
        if !self.config.endpointing_enabled || !self.config.enabled || !self.speaking || !self.user_voiced_this_turn {
            return InterruptionSignal::None;
        }
        if now_ms.saturating_sub(self.last_vad_activity_at_ms) < self.config.silence_ms {
            return InterruptionSignal::None;
        }
        self.speaking = false;
        self.user_voiced_this_turn = false;
        InterruptionSignal::SilenceEndpoint { at_ms: now_ms }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> InterruptionDetector {
        InterruptionDetector::new(InterruptionConfig::default())
    }

    #[test]
    fn ignores_voice_while_mika_is_not_speaking() {
        let mut d = detector();
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 0), InterruptionSignal::None);
    }

    #[test]
    fn ignores_voice_while_disabled() {
        let mut d = InterruptionDetector::new(InterruptionConfig { enabled: false, ..Default::default() });
        d.on_speech_started(0);
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 10), InterruptionSignal::None);
    }

    /// O caminho central: streak de fala real durante a fala da Mika dispara `BargeIn` só depois de
    /// `trigger_streak` amostras — a mesma proteção contra ruído de UMA janela que o TS tinha.
    #[test]
    fn triggers_barge_in_after_reaching_the_configured_streak() {
        let mut d = detector();
        d.on_speech_started(0);

        assert_eq!(d.feed_vad_sample(None, Some(0.9), 10), InterruptionSignal::None);
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 20), InterruptionSignal::None);
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::BargeIn { level: 0.0, at_ms: 30 });
    }

    // `speak()` publishes SpeechFinished when SYNTHESIS ends, seconds before the renderer stops
    // playing: the user talking over the rest of the answer was ignored and Mika kept talking.
    #[test]
    fn barge_in_still_fires_while_playback_outlives_synthesis() {
        let mut d = detector();
        d.on_speech_started(0);
        d.set_playback(true, 5);
        d.on_speech_finished();

        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.9), 20);
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::BargeIn { level: 0.0, at_ms: 30 });
    }

    #[test]
    fn playback_alone_arms_the_detector_and_its_end_disarms_it() {
        let mut d = detector();
        d.set_playback(true, 0);
        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.9), 20);
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::BargeIn { level: 0.0, at_ms: 30 });

        let mut d = detector();
        d.set_playback(true, 0);
        d.set_playback(false, 5);
        for t in [10, 20, 30] {
            assert_eq!(d.feed_vad_sample(None, Some(0.9), t), InterruptionSignal::None);
        }
    }

    #[test]
    fn a_low_probability_window_resets_the_streak() {
        let mut d = detector();
        d.on_speech_started(0);
        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.1), 20); // fala falha no meio do streak
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::None);
    }

    #[test]
    fn cooldown_suppresses_a_second_trigger_right_after_the_first() {
        let mut d = detector();
        d.on_speech_started(0);
        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.9), 20);
        assert!(matches!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::BargeIn { .. }));

        // `on_speech_started` de um novo turno da Mika, mas ainda dentro do cooldown de 1200ms.
        d.on_speech_started(40);
        d.feed_vad_sample(None, Some(0.9), 50);
        d.feed_vad_sample(None, Some(0.9), 60);
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 70), InterruptionSignal::None);
    }

    // --- echo grace window (defects.md "barge-in", item 4) ----------------------------------

    /// Eco típico: probabilidade morna (acima do limiar normal de 0.5, abaixo do elevado de 0.75)
    /// não pode acumular streak nos primeiros 300ms de reprodução.
    #[test]
    fn self_echo_grace_window_suppresses_a_borderline_probability_right_after_playback_starts() {
        let mut d = detector();
        d.set_playback(true, 0); // reprodução começa agora -> janela de graça até 300ms
        for t in [10, 20, 30, 40, 50] {
            assert_eq!(
                d.feed_vad_sample(None, Some(0.6), t),
                InterruptionSignal::None,
                "eco não pode acumular streak dentro da janela de graça"
            );
        }
    }

    /// Uma fala real e forte não pode ficar mais lenta por causa da janela de graça — continua
    /// disparando dentro de ~150ms (aqui, 30ms: 3 janelas de 10ms).
    #[test]
    fn a_strong_onset_still_triggers_quickly_during_the_echo_grace_window() {
        let mut d = detector();
        d.set_playback(true, 0);
        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.9), 20);
        assert_eq!(
            d.feed_vad_sample(None, Some(0.9), 30),
            InterruptionSignal::BargeIn { level: 0.0, at_ms: 30 },
            "uma interrupção real não pode ser atrasada pela janela de graça do eco"
        );
    }

    /// Passada a janela de graça, a MESMA probabilidade morna volta a valer pelo limiar normal.
    #[test]
    fn after_the_echo_grace_window_elapses_the_normal_threshold_applies_again() {
        let mut d = detector();
        d.set_playback(true, 0);
        d.feed_vad_sample(None, Some(0.6), 310);
        d.feed_vad_sample(None, Some(0.6), 320);
        assert_eq!(d.feed_vad_sample(None, Some(0.6), 330), InterruptionSignal::BargeIn { level: 0.0, at_ms: 330 });
    }

    /// `set_playback` é edge-triggered (ver o comentário do método): repetir `true` não deve reabrir
    /// a janela de graça a cada janela do VAD.
    #[test]
    fn resending_playback_true_does_not_reopen_the_grace_window() {
        let mut d = detector();
        d.set_playback(true, 0);
        d.set_playback(true, 100); // reenvio do mesmo estado — não conta como novo início
        d.feed_vad_sample(None, Some(0.6), 310);
        d.feed_vad_sample(None, Some(0.6), 320);
        assert_eq!(
            d.feed_vad_sample(None, Some(0.6), 330),
            InterruptionSignal::BargeIn { level: 0.0, at_ms: 330 },
            "310ms depois do início real (0), não de um reenvio em 100"
        );
    }

    /// Regressão: antes desta rodada `effective_speech_threshold` sempre devolvia o
    /// `NEURAL_SPEECH_THRESHOLD` fixo (0.5), então `voice.vadSpeechThreshold` salvo pela UI nunca
    /// era lido pelo detector — um limiar mais baixo configurado pelo usuário (0.2) não fazia
    /// diferença nenhuma. Com o campo `speech_threshold` no `InterruptionConfig`, uma probabilidade
    /// de 0.3 (abaixo do antigo 0.5, acima do 0.2 configurado) tem de disparar o barge-in.
    #[test]
    fn feed_vad_sample_respects_a_configured_speech_threshold_instead_of_the_hardcoded_default() {
        let mut d = InterruptionDetector::new(InterruptionConfig { speech_threshold: 0.2, ..InterruptionConfig::default() });
        d.on_speech_started(0);

        assert_eq!(d.feed_vad_sample(None, Some(0.3), 10), InterruptionSignal::None);
        assert_eq!(d.feed_vad_sample(None, Some(0.3), 20), InterruptionSignal::None);
        assert_eq!(
            d.feed_vad_sample(None, Some(0.3), 30),
            InterruptionSignal::BargeIn { level: 0.0, at_ms: 30 },
            "limiar configurado (0.2) devia ter deixado 0.3 contar como fala"
        );
    }

    #[test]
    fn falls_back_to_amplitude_threshold_without_a_neural_probability() {
        let mut d = detector();
        d.on_speech_started(0);
        d.feed_vad_sample(Some(0.2), None, 10);
        d.feed_vad_sample(Some(0.2), None, 20);
        assert!(matches!(d.feed_vad_sample(Some(0.2), None, 30), InterruptionSignal::BargeIn { .. }));
    }

    #[test]
    fn silence_endpoint_requires_prior_voiced_activity_in_this_turn() {
        let mut d = detector();
        d.on_speech_started(0);
        // Nunca houve fala real (`user_voiced_this_turn` continua falso) — sem endpoint por silêncio.
        assert_eq!(d.check_silence_endpoint(10_000), InterruptionSignal::None);
    }

    #[test]
    fn silence_endpoint_fires_after_the_configured_silence_window() {
        let mut d = detector();
        d.on_speech_started(0);
        d.feed_vad_sample(None, Some(0.9), 100); // fala real registrada, streak não importa aqui

        assert_eq!(d.check_silence_endpoint(1_000), InterruptionSignal::None);
        assert_eq!(
            d.check_silence_endpoint(1_600),
            InterruptionSignal::SilenceEndpoint { at_ms: 1_600 }
        );
    }

    #[test]
    fn on_speech_finished_resets_state_so_stale_streaks_cannot_fire_later() {
        let mut d = detector();
        d.on_speech_started(0);
        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.9), 20);
        d.on_speech_finished();
        // Streak perdido: a terceira amostra sozinha, já sem `speaking`, não dispara nada.
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::None);
    }

    #[test]
    fn update_config_resets_the_in_flight_streak() {
        let mut d = detector();
        d.on_speech_started(0);
        d.feed_vad_sample(None, Some(0.9), 10);
        d.feed_vad_sample(None, Some(0.9), 20);
        d.update_config(InterruptionConfig::default());
        // Sem o reset, a próxima amostra completaria o streak de 3 herdado de antes da mudança.
        assert_eq!(d.feed_vad_sample(None, Some(0.9), 30), InterruptionSignal::None);
    }
}
