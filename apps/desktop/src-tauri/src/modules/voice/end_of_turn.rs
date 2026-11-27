//! Decisão de fim de turno — a parte PURA e testável do classificador semântico.
//!
//! O VAD continua sendo o piso de segurança: ele corta o silêncio, mas não sabe se a frase acabou
//! ("eu queria saber se..."). Smart Turn responde a segunda pergunta com uma probabilidade. Este
//! módulo só aplica a regra e garante que NENHUMA falha prenda o usuário esperando:
//!
//! - probabilidade > limiar (0,5 por padrão) → `complete = true`;
//! - probabilidade <= limiar → `complete = false`, o renderer continua escutando;
//! - áudio vazio/curto (< 0,3 s), classificador com erro ou modelo ausente → `complete = true` com
//!   `reason` de fallback. O VAD acústico segue como piso; um classificador quebrado nunca pode ser
//!   a razão de a Mika ficar surda.
//!
//! A regra é separada do ONNX de propósito: o modelo real exige 8,7 MB baixados, e a decisão que
//! segura a conversa precisa ser provada com um classificador falso, sem rede.

use crate::modules::inference::smart_turn::SmartTurn;

/// Limiar de "turno completo". O modelo devolve probabilidade sigmoide; 0,5 é o corte documentado
/// pelo pipecat. `>`, não `>=`: no limiar exato a frase ainda é tratada como incompleta.
pub const DEFAULT_COMPLETION_THRESHOLD: f32 = 0.5;
/// Abaixo disso não há fala suficiente para classificar (0,3 s a 16 kHz).
pub const MIN_AUDIO_SAMPLES: usize = 4_800;

/// O que o classificador precisa expor. Implementado por [`SmartTurn`]; testes usam falsos.
pub trait TurnCompletionClassifier {
    fn completion_probability(&mut self, samples_16k: &[f32]) -> Result<f32, String>;
}

/// Por que a decisão foi essa — viaja para o renderer no campo `reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndOfTurnReason {
    /// O classificador respondeu; `complete` segue a probabilidade.
    Probability,
    /// Áudio curto demais para classificar.
    ShortAudio,
    /// Modelo ausente (download pendente/offline).
    ModelUnavailable,
    /// Sessão carregada, mas a inferência falhou.
    ClassifierFailed,
    /// `voice.smartTurnEnabled` desligado — a rota responde sem carregar nada.
    Disabled,
}

impl EndOfTurnReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Probability => "probability",
            Self::ShortAudio => "shortAudio",
            Self::ModelUnavailable => "modelUnavailable",
            Self::ClassifierFailed => "classifierError",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EndOfTurnDecision {
    pub complete: bool,
    pub probability: Option<f32>,
    pub reason: EndOfTurnReason,
}

impl EndOfTurnDecision {
    /// Fallback: nunca prende o usuário. O VAD acústico continua sendo o piso de segurança.
    pub fn fallback(reason: EndOfTurnReason) -> Self {
        Self { complete: true, probability: None, reason }
    }
}

/// A regra inteira. `threshold` é explícito para o teste do limiar exato; produção passa
/// [`DEFAULT_COMPLETION_THRESHOLD`].
pub fn decide_end_of_turn(
    classifier: &mut dyn TurnCompletionClassifier,
    samples_16k: &[f32],
    threshold: f32,
) -> EndOfTurnDecision {
    if samples_16k.len() < MIN_AUDIO_SAMPLES {
        return EndOfTurnDecision::fallback(EndOfTurnReason::ShortAudio);
    }

    match classifier.completion_probability(samples_16k) {
        Ok(probability) => EndOfTurnDecision {
            complete: probability > threshold,
            probability: Some(probability),
            reason: EndOfTurnReason::Probability,
        },
        Err(_) => EndOfTurnDecision::fallback(EndOfTurnReason::ClassifierFailed),
    }
}

impl TurnCompletionClassifier for SmartTurn {
    fn completion_probability(&mut self, samples_16k: &[f32]) -> Result<f32, String> {
        SmartTurn::completion_probability(self, samples_16k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeClassifier {
        outcome: Result<f32, String>,
        calls: usize,
    }

    impl FakeClassifier {
        fn new(outcome: Result<f32, String>) -> Self {
            Self { outcome, calls: 0 }
        }
    }

    impl TurnCompletionClassifier for FakeClassifier {
        fn completion_probability(&mut self, _samples_16k: &[f32]) -> Result<f32, String> {
            self.calls += 1;
            self.outcome.clone()
        }
    }

    fn audio(seconds: f32) -> Vec<f32> {
        vec![0.1; (16_000.0 * seconds) as usize]
    }

    #[test]
    fn probability_above_the_threshold_completes_the_turn() {
        let mut classifier = FakeClassifier::new(Ok(0.91));
        let decision = decide_end_of_turn(&mut classifier, &audio(1.0), DEFAULT_COMPLETION_THRESHOLD);

        assert!(decision.complete);
        assert_eq!(decision.probability, Some(0.91));
        assert_eq!(decision.reason, EndOfTurnReason::Probability);
    }

    #[test]
    fn probability_below_the_threshold_keeps_listening() {
        let mut classifier = FakeClassifier::new(Ok(0.22));
        let decision = decide_end_of_turn(&mut classifier, &audio(1.0), DEFAULT_COMPLETION_THRESHOLD);

        assert!(!decision.complete);
        assert_eq!(decision.probability, Some(0.22));
        assert_eq!(decision.reason, EndOfTurnReason::Probability);
    }

    /// O limiar é `>`, não `>=`: 0,5 exato ainda não é uma frase terminada.
    #[test]
    fn exactly_the_threshold_is_not_complete() {
        let mut classifier = FakeClassifier::new(Ok(DEFAULT_COMPLETION_THRESHOLD));
        let decision = decide_end_of_turn(&mut classifier, &audio(1.0), DEFAULT_COMPLETION_THRESHOLD);

        assert!(!decision.complete);
        assert_eq!(decision.reason, EndOfTurnReason::Probability);
    }

    #[test]
    fn a_classifier_error_falls_back_to_complete() {
        let mut classifier = FakeClassifier::new(Err("ort morreu".into()));
        let decision = decide_end_of_turn(&mut classifier, &audio(1.0), DEFAULT_COMPLETION_THRESHOLD);

        assert!(decision.complete, "erro do classificador nunca pode prender o usuário");
        assert_eq!(decision.probability, None);
        assert_eq!(decision.reason, EndOfTurnReason::ClassifierFailed);
    }

    #[test]
    fn a_missing_model_reason_still_completes_the_turn() {
        // O modelo ausente nunca chega a `decide_end_of_turn` (a rota responde antes de existir um
        // classificador), mas o fallback tem de manter a mesma garantia: nunca prender o usuário.
        let decision = EndOfTurnDecision::fallback(EndOfTurnReason::ModelUnavailable);

        assert!(decision.complete);
        assert_eq!(decision.probability, None);
        assert_eq!(decision.reason, EndOfTurnReason::ModelUnavailable);
    }

    #[test]
    fn short_audio_completes_without_calling_the_classifier() {
        let mut classifier = FakeClassifier::new(Ok(0.0));
        let decision = decide_end_of_turn(&mut classifier, &audio(0.2), DEFAULT_COMPLETION_THRESHOLD);

        assert!(decision.complete);
        assert_eq!(decision.probability, None);
        assert_eq!(decision.reason, EndOfTurnReason::ShortAudio);
        assert_eq!(classifier.calls, 0, "áudio curto não pode custar uma inferência");
    }

    #[test]
    fn empty_audio_completes_as_short_audio() {
        let mut classifier = FakeClassifier::new(Ok(0.0));
        let decision = decide_end_of_turn(&mut classifier, &[], DEFAULT_COMPLETION_THRESHOLD);

        assert!(decision.complete);
        assert_eq!(decision.reason, EndOfTurnReason::ShortAudio);
    }

    #[test]
    fn reasons_carry_the_strings_the_typescript_contract_expects() {
        for (reason, expected) in [
            (EndOfTurnReason::Probability, "probability"),
            (EndOfTurnReason::ShortAudio, "shortAudio"),
            (EndOfTurnReason::ModelUnavailable, "modelUnavailable"),
            (EndOfTurnReason::ClassifierFailed, "classifierError"),
            (EndOfTurnReason::Disabled, "disabled"),
        ] {
            assert_eq!(reason.as_str(), expected);
        }
    }
}
