//! Casos de `stream_sentences.rs`, extraídos por causa do teto de 300 linhas do AGENTS.md — mesmos
//! nomes e asserções de `apps/desktop/src/main/services/voice/audio/__tests__/stream-sentences.test.ts`.

use super::*;

fn cut(buffer: &str) -> SentenceCut {
    take_complete_sentences(buffer, false, None)
}

#[test]
fn does_not_cut_while_the_sentence_has_not_closed() {
    let result = cut("Estou pensando em como responder isso");
    assert!(result.ready.is_empty());
    assert_eq!(result.rest, "Estou pensando em como responder isso");
}

#[test]
fn cuts_on_punctuation_and_returns_the_rest_for_the_next_delta() {
    let result = cut("Isso aí é bem simples de resolver. Agora o resto");
    assert_eq!(result.ready, vec!["Isso aí é bem simples de resolver."]);
    assert_eq!(result.rest, "Agora o resto");
}

#[test]
fn a_smaller_min_chars_lets_the_first_turn_sentence_out_sooner() {
    let curto = "Ah, entendi. E aí vem o resto da resposta bem mais longa";
    assert!(take_complete_sentences(curto, false, None).ready.is_empty());
    assert_eq!(take_complete_sentences(curto, false, Some(12)).ready, vec!["Ah, entendi."]);
}

#[test]
fn does_not_cut_too_early_a_short_fragment_keeps_accumulating() {
    let result = cut("Sim. ");
    assert!(result.ready.is_empty());
    assert_eq!(result.rest, "Sim. ");
}

#[test]
fn a_line_break_also_closes_a_sentence() {
    let result = cut("Primeiro ponto que eu queria levantar\nSegundo");
    assert_eq!(result.ready, vec!["Primeiro ponto que eu queria levantar"]);
}

#[test]
fn forces_a_cut_when_the_model_overruns_the_ceiling_without_punctuating() {
    let sem_ponto = "palavra ".repeat(40);
    let result = cut(&sem_ponto);
    assert!(!result.ready.is_empty());
    assert!(result.ready[0].chars().count() <= 180);
}

#[test]
fn final_flush_delivers_the_whole_tail_even_without_punctuation() {
    let result = take_complete_sentences("sem ponto final", true, None);
    assert_eq!(result.ready, vec!["sem ponto final"]);
    assert_eq!(result.rest, "");
}

#[test]
fn final_flush_with_empty_buffer_never_emits_empty_speech() {
    assert_eq!(take_complete_sentences("", true, None), SentenceCut::default());
    assert_eq!(take_complete_sentences("   ", true, None), SentenceCut::default());
}

#[test]
fn never_splits_a_word_in_the_forced_cut() {
    let corrido = format!("{}fim", "palavra ".repeat(40));
    let result = cut(&corrido);
    for chunk in result.ready {
        assert!(!chunk.ends_with("palav") && !chunk.ends_with("palavr"));
    }
}

#[test]
fn treats_ellipsis_as_end_of_sentence() {
    let result = cut("Nossa, sei lá, acho que talvez… Mas enfim");
    assert_eq!(result.ready, vec!["Nossa, sei lá, acho que talvez…"]);
}

#[test]
fn preserves_the_whole_text_across_a_chunked_stream() {
    let completo =
        "Oi! Tudo bem com você hoje? Nossa, que legal esse jogo. Eu tô vendo sua tela agora e achei excelente.";
    let chars: Vec<char> = completo.chars().collect();
    let mut buffer = String::new();
    let mut falado: Vec<String> = Vec::new();

    for piece in chars.chunks(7) {
        buffer.push_str(&piece.iter().collect::<String>());
        let result = cut(&buffer);
        falado.extend(result.ready);
        buffer = result.rest;
    }
    let result = take_complete_sentences(&buffer, true, None);
    falado.extend(result.ready);

    let joined = falado.join(" ");
    let collapsed: String = joined.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(collapsed, completo);
}

#[test]
fn does_not_cut_a_title_even_past_the_char_floor() {
    let result = cut("Ontem de manha eu conversei com o Dr. Silva sobre o teu patch.");
    assert!(result.ready.is_empty());
    assert_eq!(result.rest, "Ontem de manha eu conversei com o Dr. Silva sobre o teu patch.");
}

#[test]
fn holds_for_other_titles_with_and_without_uppercase() {
    for abrev in ["Sr.", "sra.", "Profa.", "Eng."] {
        let frase = format!("A gente marcou pra terca as 15h com o {abrev} Antonio na sala tres.");
        assert!(cut(&frase).ready.is_empty(), "abreviatura {abrev} não devia fechar frase");
    }
}

#[test]
fn an_isolated_initial_does_not_close_a_sentence() {
    let result = cut("O autor preferido dele eh o J. R. R. Tolkien, sem duvida nenhuma");
    assert!(result.ready.is_empty());
}

#[test]
fn a_real_sentence_after_an_abbreviation_still_closes_normally() {
    let result = cut("Ontem eu conversei com o Dr. Silva sobre isso. E agora o resto");
    assert_eq!(result.ready, vec!["Ontem eu conversei com o Dr. Silva sobre isso."]);
    assert_eq!(result.rest, "E agora o resto");
}

#[test]
fn open_reasoning_block_retains_everything_until_it_closes() {
    let result = cut("<think>Preciso lembrar do patch. Ele foi aprovado ontem.");
    assert!(result.ready.is_empty());
    assert_eq!(result.rest, "<think>Preciso lembrar do patch. Ele foi aprovado ontem.");
}

#[test]
fn a_closed_block_disappears_and_only_the_real_speech_is_delivered() {
    let result =
        cut("<think>Preciso lembrar do patch. Ele foi aprovado ontem.</think> O teu patch ja esta pronto. E o resto");
    assert_eq!(result.ready, vec!["O teu patch ja esta pronto."]);
}

#[test]
fn text_before_an_open_block_is_still_spoken() {
    let result = cut("Deixa eu ver isso aqui com calma. <think>o patch de ontem");
    assert_eq!(result.ready, vec!["Deixa eu ver isso aqui com calma."]);
}

#[test]
fn an_open_block_at_the_end_of_the_turn_is_discarded_never_spoken() {
    let result = take_complete_sentences("Ja te respondo. <think>ainda estou pensando no", true, None);
    assert_eq!(result.ready, vec!["Ja te respondo."]);
    assert_eq!(result.rest, "");
}

#[test]
fn flush_speakable_releases_the_sentence_waiting_for_the_next_delta() {
    let result = flush_speakable("Deixa eu ver isso.");
    assert_eq!(result, SentenceCut { ready: vec!["Deixa eu ver isso.".to_string()], rest: String::new() });
}

#[test]
fn flush_speakable_with_an_empty_buffer_produces_no_speech() {
    assert_eq!(flush_speakable("   "), SentenceCut::default());
}

#[test]
fn flush_speakable_holds_an_open_reasoning_block() {
    let result = flush_speakable("Vou olhar isso. <think>preciso lembrar do");
    assert_eq!(result.ready, vec!["Vou olhar isso."]);
    assert_eq!(result.rest, "<think>preciso lembrar do");
}

// -- cerca de código: uma cerca aberta retém tudo; uma fechada nunca é cortada por dentro ----------

#[test]
fn an_open_code_fence_retains_everything_until_it_closes() {
    let result = cut("Deixa eu te mostrar como funciona isso. ```js\nfunction f() {\n  return 1;\n}");
    assert_eq!(result.ready, vec!["Deixa eu te mostrar como funciona isso."]);
    assert_eq!(result.rest, "```js\nfunction f() {\n  return 1;\n}");
}

#[test]
fn a_code_fence_that_closes_in_a_later_delta_is_released_as_one_piece() {
    let mut buffer = "Deixa eu te mostrar como funciona isso. ```js\nfunction f() {\n  return 1;\n}".to_string();
    let first = cut(&buffer);
    assert_eq!(first.ready, vec!["Deixa eu te mostrar como funciona isso."], "a cerca ainda aberta não pode vazar");
    buffer = first.rest;

    buffer.push_str("\n```");
    let second = take_complete_sentences(&buffer, true, None);
    assert_eq!(second.ready, vec!["```js\nfunction f() {\n  return 1;\n}\n```"], "fechada, sai inteira — nunca em pedaços");
}

#[test]
fn a_multiline_closed_fence_is_never_cut_on_its_internal_newlines() {
    let result = cut("```js\nfunction f() {\n  return 1;\n}\n``` Beleza, era só isso. E o resto");
    // As quebras de linha DENTRO da cerca não podem contar como fim de frase — só a pontuação
    // depois dela.
    assert_eq!(result.ready, vec!["```js\nfunction f() {\n  return 1;\n}\n``` Beleza, era só isso."]);
    assert_eq!(result.rest, "E o resto");
}

#[test]
fn a_long_closed_fence_is_force_flushed_whole_instead_of_being_cut_inside() {
    let code = format!("```\n{}\n```", "        linha_de_codigo_bem_indentada();\n".repeat(6));
    assert!(code.chars().count() > 180, "o teste só prova algo se o bloco estourar o teto normal");

    let result = cut(&format!("{code} E aí, entendeu"));

    assert_eq!(result.ready, vec![code], "a cerca inteira é empurrada de uma vez, nunca partida no meio");
    assert_eq!(result.rest, "E aí, entendeu");
}

// Primeiro áudio do turno: a primeira ORAÇÃO sai na vírgula quando a frase ainda não fechou — o
// resto do turno continua cortando só em fim de frase (`take_complete_sentences`).
#[test]
fn the_first_clause_leaves_at_a_comma_before_the_sentence_closes() {
    let result = take_first_clause("Nossa, que legal que você passou dessa fase, eu sabia que");
    assert_eq!(result.ready, vec!["Nossa, que legal que você passou dessa fase,"]);
    assert_eq!(result.rest, "eu sabia que");
}

#[test]
fn a_short_opening_clause_waits_for_more_text() {
    let result = take_first_clause("Hmm, deixa eu ver aqui");
    assert!(result.ready.is_empty(), "\"Hmm,\" sozinho sairia picotado: {:?}", result.ready);
    assert_eq!(result.rest, "Hmm, deixa eu ver aqui");
}

#[test]
fn a_decimal_comma_is_not_a_clause_boundary() {
    let result = take_first_clause("O patch novo deixou o dano 1,5 vezes maior que antes");
    assert!(result.ready.is_empty(), "{:?}", result.ready);
}

#[test]
fn a_clause_inside_an_open_reasoning_block_is_never_spoken() {
    let buffer = "<think>vou pensar com bastante calma, sobre isso tudo";
    let result = take_first_clause(buffer);
    assert!(result.ready.is_empty(), "{:?}", result.ready);
    assert_eq!(result.rest, buffer);
}

#[test]
fn cjk_full_stop_closes_a_sentence_without_a_following_space() {
    let ja = "今日はとても良い天気ですね、散歩に行きましょう。次の話をします";
    let result = cut(ja);
    assert_eq!(result.ready, vec!["今日はとても良い天気ですね、散歩に行きましょう。"]);
    assert_eq!(result.rest, "次の話をします");

    let zh = "我今天去了很远的地方看望很多年没有见面的老朋友，他们都非常热情！接下来的故事";
    assert_eq!(cut(zh).ready, vec!["我今天去了很远的地方看望很多年没有见面的老朋友，他们都非常热情！"]);
}

#[test]
fn cjk_closing_quote_stays_with_its_sentence() {
    let result = cut("彼は静かに笑ってから優しく言いました「また明日の朝にここで会いましょう。」それから眠りました");
    assert_eq!(result.ready, vec!["彼は静かに笑ってから優しく言いました「また明日の朝にここで会いましょう。」"]);
    assert_eq!(result.rest, "それから眠りました");
}

#[test]
fn first_clause_cuts_at_a_cjk_comma_without_a_space() {
    let result = take_first_clause("今日はとても良い天気なので友達と一緒に近くの公園まで歩いて行くことにした、それから");
    assert_eq!(result.ready, vec!["今日はとても良い天気なので友達と一緒に近くの公園まで歩いて行くことにした、"]);
    assert_eq!(result.rest, "それから");
}
