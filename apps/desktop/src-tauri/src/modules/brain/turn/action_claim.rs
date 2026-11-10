//! Detector puro: uma resposta final que AFIRMA ter executado uma ação no PC agora ("abrindo a
//! calculadora", "cliquei", "pesquisei", "rodei o código", "executei"...) sem nenhuma tool chamada
//! neste turno, respondendo a um pedido de ação do próprio usuário.
//!
//! Achado real (smoke item 6): "abre a calculadora" -> Gemma respondeu "Abrindo a calculadora
//! agora" com ZERO tool call; a calculadora nunca abriu. Sem rede, sem `ConfigStore`, sem
//! `regex` compilada a cada chamada (`OnceLock`, mesmo padrão de `post_turn/session_accumulator.rs`)
//! — `brain::turn` usa [`claims_untooled_action`] pra decidir se força uma rodada corretiva antes
//! de aceitar o texto como resposta final (ver o cabeçalho de `turn.rs`).

use regex::Regex;
use std::sync::OnceLock;

/// Pedido de ação dirigido à Mika, imperativo (2ª pessoa informal e formal) — heurística por
/// palavra inteira, não NLU: cobre os pedidos do smoke ("abre a calculadora", "clica no botão",
/// "pesquisa na internet", "roda o código"/"escreve ... e roda", "executa o script").
fn action_request_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(abre|abra|clica|clique|pesquisa|pesquise|busca|busque|roda|rode|executa|execute|digita|digite)\b").unwrap())
}

/// Afirmação de ação já feita — 1ª pessoa, passado/gerúndio/presente-contínuo. Nunca inclui
/// futuro/intenção ("vou abrir"): `\babri\b` não casa dentro de "abrir" (sem fronteira de palavra
/// entre "abri" e o "r" seguinte), então intenção nunca vira falso positivo de conclusão.
fn action_claim_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(abrindo|abri|cliquei|clicando|pesquisei|pesquisando|busquei|buscando|rodei|rodando|executei|executando|digitei|digitando)\b",
        )
        .unwrap()
    })
}

/// Guarda de idioma: "abri meu coração"/"abri a mente"/"abri os braços" não são uma ação de PC.
/// Lista literal do que já apareceu, sem pretensão de exaustividade (é heurística, não NLU).
fn idiomatic_abri_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)abri\s+(o\s+|a\s+|meu\s+|minha\s+|os\s+|as\s+)?(cora[çc][ãa]o|mente|bra[çc]os|m[ãa]o)\b").unwrap())
}

/// O usuário pediu uma ação de PC nesta mensagem (heurística por verbo imperativo).
pub fn user_requested_an_action(user_content: &str) -> bool {
    action_request_re().is_match(user_content)
}

/// O texto da Mika afirma ter executado a ação agora — descontado o idioma "abri meu coração".
pub fn assistant_claims_action_done(assistant_text: &str) -> bool {
    action_claim_re().is_match(assistant_text) && !idiomatic_abri_re().is_match(assistant_text)
}

/// Promise of the action ("vou abrir"). Only a claim in the FINAL reply: with zero tool calls the
/// turn ends there, so the promise is never kept.
fn action_promise_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bvou\s+(abrir|clicar|pesquisar|buscar|rodar|executar|digitar)\b").unwrap())
}
/// A checagem completa que `brain::turn` chama: pedido de ação + afirmação de feito (ou promessa),
/// na MESMA dupla usuário/assistente. Quem chama já garante "zero tool call neste turno" (não é
/// algo que este módulo puro possa saber sozinho).
pub fn claims_untooled_action(user_content: &str, assistant_text: &str) -> bool {
    user_requested_an_action(user_content) && (assistant_claims_action_done(assistant_text) || action_promise_re().is_match(assistant_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_real_smoke_scenario() {
        assert!(claims_untooled_action("abre a calculadora", "Abrindo a calculadora agora!"));
        assert!(claims_untooled_action("pesquisa na internet sobre Elden Ring", "Já pesquisei e encontrei isso pra você!"));
        assert!(claims_untooled_action("escreve um código JS e roda", "Rodei o código e deu 42."));
        assert!(claims_untooled_action("clica no botão 7", "Cliquei ali, deu certo!"));
        assert!(claims_untooled_action("executa esse script pra mim", "Executei o script com sucesso."));
    }

    #[test]
    fn does_not_fire_on_plain_explanatory_text() {
        assert!(!assistant_claims_action_done("Vou te explicar como isso funciona."));
        assert!(!claims_untooled_action("abre a calculadora", "Vou te explicar como abrir a calculadora."));
    }

    #[test]
    fn does_not_fire_on_the_abri_idiom() {
        assert!(!assistant_claims_action_done("Abri meu coração pra você hoje."));
        assert!(!assistant_claims_action_done("Abri a mente sobre esse assunto."));
    }

    #[test]
    fn future_intent_is_not_a_completion_claim() {
        assert!(!assistant_claims_action_done("Vou abrir a calculadora."));
        assert!(!assistant_claims_action_done("Posso pesquisar isso, quer que eu faça?"));
    }

    /// Smoke 2026-09-26: "Vou abrir a calculadora pra você agora, beleza?" was the whole final reply,
    /// zero tool calls — the turn ended on a promise nothing would keep.
    #[test]
    fn a_final_reply_that_only_promises_the_action_fires_too() {
        assert!(claims_untooled_action("abre a calculadora", "<|WAVE|> Vou abrir a calculadora pra você agora, beleza?"));
        assert!(claims_untooled_action("roda um código JS", "Tá bom, vou rodar esse desafio aqui agora."));
        assert!(!claims_untooled_action("abre a calculadora", "Vou te explicar como abrir a calculadora."));
        assert!(!claims_untooled_action("oi", "Vou abrir o jogo contigo: tô cansada."));
    }

    #[test]
    fn a_claim_without_a_matching_user_request_does_not_fire() {
        // A Mika mencionar que "cliquei" numa piada sobre outra coisa, sem o usuário ter pedido
        // ação nenhuma nesta mensagem, não é o cenário que este detector existe pra pegar.
        assert!(!claims_untooled_action("oi, tudo bem?", "Cliquei sem querer no seu coração, haha."));
    }

    #[test]
    fn user_requested_an_action_recognizes_the_smoke_requests() {
        for request in ["abre a calculadora", "clica no botão 7", "pesquisa na internet", "roda o código", "executa isso"] {
            assert!(user_requested_an_action(request), "deveria reconhecer pedido de ação: {request}");
        }
        assert!(!user_requested_an_action("oi, tudo bem?"));
    }
}
