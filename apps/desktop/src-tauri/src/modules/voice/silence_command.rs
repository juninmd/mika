//! `mika:voice-silence-command` — o caminho de voz de `specs/08-gamer-companion.md` ("Silêncio é
//! feature"), que a spec marcava "verificado, ausente": a tool `voice` (ação `mute`,
//! [`super::voice_tool`]) já publicava `EventKey::MuteSpeech` de verdade, mas só atrás de um turno
//! de LLM inteiro — pedir silêncio esperava a Mika terminar de pensar (e às vezes de falar) antes
//! de calar. Este módulo intercepta o transcript FINAL, antes do turno começar: se for um comando
//! de silêncio, publica `MuteSpeech` direto no barramento e devolve `matched: true` para o
//! renderer NÃO chamar `mika:chat-send` com essa fala.
//!
//! **Casamento conservador, de propósito.** Frase inteira normalizada, ou comando NO INÍCIO da fala
//! (`"cala a boca, por favor"` casa; a fala inteira "por favor cala a boca" não, porque o comando
//! não abre a frase — mesmo critério usado para não confundir "cala a boca" com um pedido educado
//! em volta dele). Nunca substring solta no meio: `"eu gosto de silêncio total"` NÃO pode disparar
//! só por conter "silêncio" — [`detect`] teria de aceitar substring para isso acontecer, e não
//! aceita. [`normalize`] derruba maiúscula, acento e pontuação antes de comparar: o STT real varia
//! esses três eixos entre chamadas do MESMO comando (mesma razão de `transcribe.rs::is_pcm16_mime`
//! comparar por prefixo, não igualdade exata, mas aqui o eixo variável é texto, não mimeType).
//!
//! **Duração idêntica ao atalho.** `crate::shortcuts::MUTE_WINDOW_MS` (15 minutos) é a MESMA janela
//! de `CommandOrControl+Shift+S` — um comando de voz não é mais nem menos definitivo que o atalho de
//! teclado, então não inventa um prazo próprio.
//!
//! **Não abortar o turno.** Ao contrário do barge-in (`vad_audio.rs::react_to_barge_in`, que aborta
//! um turno JÁ em curso), aqui não existe turno para abortar: o comando substitui o transcript ANTES
//! de qualquer turno nascer. Só falta calar quem já está falando — a UI trata isso separado (chama
//! `stopTTS()` local, mesmo caminho que já usa antes de mandar qualquer fala nova enquanto a Mika
//! fala, ver `useShellVoice.ts`), porque parar a REPRODUÇÃO de áudio já em andamento é local ao
//! player do renderer, sem estado nenhum que este módulo precise tocar.

use serde_json::{json, Value};

use crate::events::{bus, EventKey};

/// Comandos de calar — pt-BR primeiro (idioma padrão do app), depois os locais que
/// `apps/desktop/src/renderer/src/components/wizard/copy/*.ts` também traduz (en-us, es-es, fr-fr).
/// Já normalizados (minúsculo, sem acento/pontuação) — [`detect`] normaliza a entrada com a MESMA
/// função antes de comparar.
const MUTE_PHRASES: &[&str] = &[
    "cala a boca",
    "cale a boca",
    "cale se",
    "fica quieta",
    "fique quieta",
    "silencio",
    "psiu",
    "shh",
    "shush",
    "shut up",
    "be quiet",
    "quiet",
    "hush",
    "callate",
    "callate la boca",
    "calla",
    "tais toi",
    "chut",
];

/// Simétrico a [`MUTE_PHRASES`] — SHOULD do brief, não MUST: desmutar por voz é conveniência, não a
/// garantia de segurança que "silenciar instantaneamente" é.
const UNMUTE_PHRASES: &[&str] = &[
    "pode falar",
    "fala de novo",
    "volta a falar",
    "desmuda",
    "you can talk",
    "talk again",
    "unmute",
    "speak again",
    "puedes hablar",
    "habla de nuevo",
    "tu peux parler",
    "parle encore",
];

/// Minúsculo, sem acento, só letras/números/espaço, espaços colapsados e aparados — mesma fala dita
/// duas vezes pelo Whisper raramente sai com a MESMA pontuação/capitalização; comparar depois de
/// normalizar é o que torna "Cala a boca!" e "cala a boca" o mesmo comando.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_was_space = false;
    for ch in text.chars() {
        let folded = strip_accent(ch.to_lowercase().next().unwrap_or(ch));
        if folded.is_ascii_alphanumeric() {
            out.push(folded);
            last_was_space = false;
        } else if !last_was_space {
            out.push(' ');
            last_was_space = true;
        }
    }
    out.trim().to_string()
}

/// Só os acentos que os quatro locais suportados (pt/en/es/fr) realmente usam — não é um
/// desacentuador Unicode genérico, é o suficiente para as frases de [`MUTE_PHRASES`]/[`UNMUTE_PHRASES`].
fn strip_accent(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        other => other,
    }
}

/// `true` quando `phrase` é a frase INTEIRA normalizada, ou abre `normalized` seguida de fronteira
/// de palavra (espaço, já que `normalize` não deixa outro separador sobrar) — nunca substring solta
/// no meio. `"silencio"` casa `"silencio"` e `"silencio por favor"`, mas não `"o silencio e bom"`
/// (o comando não abre a frase) nem `"silenciosamente"` (não há fronteira de palavra depois).
fn matches_leading(normalized: &str, phrase: &str) -> bool {
    if normalized == phrase {
        return true;
    }
    normalized.strip_prefix(phrase).is_some_and(|rest| rest.starts_with(' '))
}

/// `Some(true)` = comando de calar, `Some(false)` = comando de desmutar, `None` = fala comum (segue
/// para o turno normal). Testa mudo antes de desmudo só porque [`MUTE_PHRASES`] é a lista MUST deste
/// recurso; nenhuma frase aparece nas duas listas, então a ordem não decide empate nenhum de verdade.
pub(crate) fn detect(text: &str) -> Option<bool> {
    let normalized = normalize(text);
    if normalized.is_empty() {
        return None;
    }
    if MUTE_PHRASES.iter().any(|phrase| matches_leading(&normalized, phrase)) {
        return Some(true);
    }
    if UNMUTE_PHRASES.iter().any(|phrase| matches_leading(&normalized, phrase)) {
        return Some(false);
    }
    None
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Mesma forma de payload que `shortcuts.rs::next_mute_payload` publica para o atalho de teclado —
/// um assinante de `MuteSpeech` (`speech_mute::apply`) não pode ver diferença entre as duas origens.
fn mute_payload() -> Value {
    json!({ "muted": true, "untilMs": now_ms() + crate::shortcuts::MUTE_WINDOW_MS })
}

fn unmute_payload() -> Value {
    json!({ "muted": false })
}

/// Payload de `mika:voice-silence-command`: só `text`, o transcript final que o renderer já decidiu
/// mandar para o turno — o mesmo texto que iria para `sendMessage`, ANTES de mandar.
fn parse_text(payload: &Value) -> Result<&str, String> {
    payload["text"].as_str().ok_or_else(|| "'text' is missing or not a string".to_string())
}

/// Ponto de entrada chamado por `voice/mod.rs::VoiceModule::invoke` — sem guarda de superfície
/// própria: já roda atrás da MESMA guarda que `mika:voice-transcribe`/`voice:vad-audio` (só
/// main/settings/wizard produzem transcript de voz para começar).
///
/// Nunca propaga erro de negócio: um comando não reconhecido é `{"matched": false}`, não uma falha
/// — mesmo contrato de `transcribe::handle` (toda degradação de ASR vira valor, não `Err`).
pub(crate) fn handle(payload: &Value) -> Result<Value, String> {
    let text = parse_text(payload)?;
    match detect(text) {
        Some(true) => {
            bus().publish(EventKey::MuteSpeech, mute_payload());
            Ok(json!({ "matched": true, "muted": true }))
        }
        Some(false) => {
            bus().publish(EventKey::MuteSpeech, unmute_payload());
            Ok(json!({ "matched": true, "muted": false }))
        }
        None => Ok(json!({ "matched": false })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::recv_matching;
    use std::time::Duration;

    #[test]
    fn normalize_folds_case_accents_and_punctuation() {
        assert_eq!(normalize("Cala a Boca!!"), "cala a boca");
        assert_eq!(normalize("Silêncio, por favor."), "silencio por favor");
        assert_eq!(normalize("  Fica   quieta   "), "fica quieta");
        assert_eq!(normalize("Cállate, ya"), "callate ya");
    }

    /// `to_ascii_lowercase` deixava `Ê`/`Á` maiúsculos fora da tabela de acentos: "SILÊNCIO" virava
    /// "sil ncio" e o comando de calar não casava (o mesmo para "CÁLLATE").
    #[test]
    fn normalize_folds_uppercase_accented_letters_too() {
        assert_eq!(normalize("SILÊNCIO!"), "silencio");
        assert_eq!(detect("SILÊNCIO"), Some(true));
        assert_eq!(detect("CÁLLATE"), Some(true));
    }

    #[test]
    fn normalize_of_empty_or_punctuation_only_is_empty() {
        assert_eq!(normalize(""), "");
        assert_eq!(normalize("...!?"), "");
    }

    // --- MUST: detecção conservadora --------------------------------------------------------

    #[test]
    fn whole_utterance_mute_commands_match_in_every_supported_locale() {
        assert_eq!(detect("cala a boca"), Some(true));
        assert_eq!(detect("Cala a Boca!"), Some(true));
        assert_eq!(detect("fica quieta"), Some(true));
        assert_eq!(detect("silêncio"), Some(true));
        assert_eq!(detect("shh"), Some(true));
        assert_eq!(detect("shut up"), Some(true));
        assert_eq!(detect("quiet"), Some(true));
        assert_eq!(detect("cállate"), Some(true));
        assert_eq!(detect("tais-toi"), Some(true));
        assert_eq!(detect("chut"), Some(true));
    }

    #[test]
    fn leading_mute_command_matches_with_trailing_words() {
        assert_eq!(detect("cala a boca, por favor"), Some(true));
        assert_eq!(detect("shut up already"), Some(true));
        assert_eq!(detect("silêncio, estou tentando ouvir"), Some(true));
    }

    /// O caso negativo central do brief: o comando NO MEIO da frase não pode disparar.
    #[test]
    fn a_command_word_buried_mid_sentence_never_matches() {
        assert_eq!(detect("eu gosto de silêncio total"), None);
        assert_eq!(detect("por favor cala a boca"), None, "comando não abre a frase — não é leading");
        assert_eq!(detect("o silêncio da noite é bonito"), None);
    }

    /// Fronteira de palavra de verdade: prefixo textual sem separador não é o comando.
    #[test]
    fn a_command_as_a_prefix_of_a_longer_word_never_matches() {
        assert_eq!(detect("silenciosamente ela saiu"), None);
        assert_eq!(detect("quietude é rara"), None);
    }

    #[test]
    fn ordinary_speech_never_matches() {
        assert_eq!(detect("como você está hoje?"), None);
        assert_eq!(detect(""), None);
        assert_eq!(detect("   "), None);
    }

    #[test]
    fn unmute_commands_match_symmetrically() {
        assert_eq!(detect("pode falar"), Some(false));
        assert_eq!(detect("you can talk again"), Some(false));
        assert_eq!(detect("Fala de novo!"), Some(false));
    }

    // --- o funnel: publica MuteSpeech e sinaliza matched, sem duplicar como forward ----------
    //
    // `bus()` é singleton de processo e `mute_learning.rs` (outro arquivo) também publica
    // `MuteSpeech` de verdade nos seus próprios testes — `recv_matching` filtra só pela CHAVE, não
    // pelo PAYLOAD, então duas chamadas concorrentes à mesma chave podem entregar o envelope uma da
    // outra (ver o aviso em `events/bus.rs`). [`drain_until`] resolve isto do jeito recomendado lá
    // (opção 2, adaptada): continua drenando `MuteSpeech` até achar um payload que bata com o
    // ESPERADO, ou o prazo esgotar — nunca aceita "o primeiro que chegou" como se fosse o nosso.
    async fn drain_until(sub: &mut crate::events::EventSubscription, deadline: Duration, matches: impl Fn(&Value) -> bool) -> bool {
        let until = tokio::time::Instant::now() + deadline;
        loop {
            let remaining = until.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            match recv_matching(sub, EventKey::MuteSpeech, remaining).await {
                Some(envelope) if matches(&envelope.payload) => return true,
                Some(_) => continue, // MuteSpeech de outro teste concorrente — não é o nosso, continua drenando
                None => return false,
            }
        }
    }

    #[tokio::test]
    async fn a_matched_mute_command_publishes_mute_speech_with_the_shortcut_window() {
        let mut sub = bus().subscribe();
        let before = now_ms();

        let result = handle(&json!({ "text": "cala a boca" })).unwrap();
        assert_eq!(result, json!({ "matched": true, "muted": true }));

        let found = drain_until(&mut sub, Duration::from_millis(500), |payload| {
            payload["muted"] == json!(true)
                && payload["untilMs"].as_i64().is_some_and(|until_ms| until_ms >= before + crate::shortcuts::MUTE_WINDOW_MS)
        })
        .await;
        assert!(found, "MuteSpeech com a janela do comando de voz devia aparecer no barramento real");
    }

    #[tokio::test]
    async fn a_matched_unmute_command_publishes_muted_false() {
        let mut sub = bus().subscribe();

        let result = handle(&json!({ "text": "pode falar" })).unwrap();
        assert_eq!(result, json!({ "matched": true, "muted": false }));

        let found = drain_until(&mut sub, Duration::from_millis(500), |payload| *payload == json!({ "muted": false })).await;
        assert!(found, "MuteSpeech{{muted:false}} devia aparecer no barramento real");
    }

    /// A prova de "não forwarding": um transcript comum devolve `matched: false` — é ISSO que o
    /// renderer lê para decidir se manda a fala para o turno de LLM (`useShellVoice.ts::onTranscript`).
    /// Não dá para também provar "nada foi publicado" contra `bus()` aqui: os dois testes ACIMA,
    /// neste MESMO arquivo, publicam `MuteSpeech` de verdade e podem rodar em paralelo com este
    /// (mesmo aviso de `events/bus.rs`, opção 3) — o contrato que o renderer depende é só o valor de
    /// retorno, que é o que esta asserção prova.
    #[tokio::test]
    async fn ordinary_speech_never_matches_and_never_forwards() {
        let result = handle(&json!({ "text": "como foi seu dia hoje?" })).unwrap();
        assert_eq!(result, json!({ "matched": false }));
    }

    #[test]
    fn parse_text_requires_a_string_field() {
        assert!(parse_text(&json!({})).is_err());
        assert!(parse_text(&json!({ "text": 42 })).is_err());
        assert_eq!(parse_text(&json!({ "text": "oi" })).unwrap(), "oi");
    }
}
