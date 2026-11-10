use super::*;

#[test]
fn short_real_speech_is_plausible() {
    assert!(!is_implausible_transcript("oi tudo bem com voce", Some(2_000), 0));
}

#[test]
fn degenerate_repetition_is_always_implausible_regardless_of_duration() {
    let text = "vai vai vai vai vai vai vai vai vai vai vai vai";
    assert!(is_implausible_transcript(text, None, 0));
}

#[test]
fn missing_duration_never_condemns() {
    assert!(!is_implausible_transcript("qualquer coisa aqui", None, 0));
}

#[test]
fn too_short_to_measure_is_never_condemned() {
    assert!(!is_implausible_transcript("oi", Some(500), 0));
}

#[test]
fn low_density_long_text_is_implausible() {
    // Texto longo, duração curtíssima -> densidade absurda (> MAX_CHARS_PER_SECOND).
    let text = "a".repeat(200);
    assert!(is_implausible_transcript(&text, Some(1_500), 0));
}

#[test]
fn sparse_short_window_with_filler_min_zero_condemns_stock_phrase() {
    // Mesmo cenário do barge-in real: 2.5s de captura, "tchau" solto no meio de silêncio.
    assert!(is_implausible_transcript("tchau", Some(2_500), 0));
}

#[test]
fn filler_phrase_is_plausible_below_the_hallucination_floor_when_min_filler_ms_is_high() {
    // Com o piso do STT contínuo (6s, o `FILLER_MIN_HALLUCINATION_MS` que
    // `interruption-transcript.ts` usa quando NÃO é o barge-in chamando), a mesma frase curta
    // não é condenada por estar abaixo da janela mínima de alucinação.
    assert!(!is_implausible_transcript("tchau", Some(2_500), 6_000));
}

#[test]
fn hallucination_log_message_names_the_active_provider_not_a_fixed_whisper() {
    // Regressão: a linha era hardcoded "alucinação do Whisper" mesmo com voice.asrProvider =
    // nemotron, mandando quem depurasse o log investigar o motor errado.
    let msg = hallucination_log_message("tchau", 2_500, "nemotron");
    assert!(msg.contains("nemotron hallucination"), "esperava o provedor real na mensagem: {msg}");
    assert!(!msg.contains("Whisper hallucination"), "não deveria mais fixar \"Whisper\": {msg}");
}

#[test]
fn i16_to_le_bytes_round_trips_sign() {
    let bytes = i16_to_le_bytes(&[-1, 0, 1]);
    assert_eq!(bytes, vec![0xFF, 0xFF, 0x00, 0x00, 0x01, 0x00]);
}

/// Porte de `if (stt.isListening()) return ''`: sai ANTES de chamar o Whisper — por isso este
/// teste não precisa do modelo real, só do estado global de escuta ligado.
/// `listening_state::global()` é PROCESSO INTEIRO (ver o cabeçalho do arquivo), então este
/// teste limpa o próprio rastro no fim para não vazar `listening=true` para outro teste deste
/// binário rodando depois.
#[tokio::test]
async fn process_interruption_audio_skips_transcription_when_the_continuous_stt_is_listening() {
    listening_state::global().start();

    let audio = InterruptionAudio { samples: vec![0i16; 512], sample_rate: 16_000 };
    let result =
        process_interruption_audio(std::path::PathBuf::new(), audio, Some("pt".to_string()), "nemotron", "onnx-community/whisper-base", 0)
            .await;

    listening_state::global().stop();
    assert_eq!(result, None, "com o STT contínuo ouvindo, o barge-in não deve transcrever nem publicar nada");
}
