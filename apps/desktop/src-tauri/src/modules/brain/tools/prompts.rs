//! Prompts markdown de `ContentPipelineService`/`CollaborationService`
//! (`apps/desktop/src/main/services/brain/prompts/{daily-recap,social-post,collaboration-opening,
//! collaboration-turn}.md`), embutidos em tempo de COMPILAÇÃO — não em runtime, ao contrário de
//! `reflection.rs::load_prompt`/`chat/prompt.rs`.
//!
//! Por quê diferente do loader runtime de `reflection.rs`: aquele resolve `prompts` via
//! `AppHandle` (bundle empacotado + override do usuário). `content`/`collaboration` são registrados
//! em `register_config_bound_tools(store: Arc<ConfigStore>)` (`tools/mod.rs`) — SEM `AppHandle`
//! disponível nesse ponto de boot; a assinatura de quem chama vem de `modules/mod.rs`, fora do
//! escopo desta rodada. `include_str!` resolve isso sem tocar naquele arquivo, e falha em tempo de
//! COMPILAÇÃO se o `.md` sumir — garantia MAIS forte que o loader runtime: nunca dá pra publicar um
//! binário com um destes 4 prompts ausente ou vazio (a classe de defeito "prompt vazio → request de
//! zero token → 400 → provedor desativado" já registrada neste repositório). O preço documentado:
//! perde o diretório de override do usuário (`prompt-overrides/`) que o loader runtime respeita —
//! ninguém troca estes 4 prompts sem recompilar. Aceito para esta rodada; se um dia
//! `content`/`collaboration` precisarem do override também, é o mesmo dia em que vale plumbar
//! `AppHandle` até aqui.
//!
//! Profundidade contada, não copiada do vizinho: este arquivo mora em
//! `src-tauri/src/modules/brain/tools/`, mesma profundidade de
//! `src-tauri/src/modules/brain/settings/brain_tab.rs` (que usa
//! `"../../../../../src/main/services/brain/brain.schema.ts"`, 5 `../` até `apps/desktop/`) — os
//! mesmos 5 níveis abaixo.

use std::collections::HashMap;

pub(super) const DAILY_RECAP: &str = include_str!("../../../../../prompts/brain/daily-recap.md");
pub(super) const SOCIAL_POST: &str = include_str!("../../../../../prompts/brain/social-post.md");
pub(super) const COLLABORATION_OPENING: &str =
    include_str!("../../../../../prompts/brain/collaboration-opening.md");
pub(super) const COLLABORATION_TURN: &str =
    include_str!("../../../../../prompts/brain/collaboration-turn.md");

/// Preenche `{{placeholder}}` (`\w+` ASCII) — mesmo contrato de `renderTemplate`
/// (`packages/sdk/src/prompts/loader.ts`) e o mesmo padrão já duplicado em `chat/prompt.rs`/
/// `reflection.rs` (motivo idêntico: a função de lá não é `pub`, e este arquivo só toca a si
/// mesmo). Um placeholder sem valor em `vars` vira string vazia, com aviso — nunca panica.
pub(super) fn render(template: &str, vars: &[(&str, &str)]) -> String {
    let map: HashMap<&str, &str> = vars.iter().copied().collect();
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::with_capacity(template.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '{' && i + 1 < chars.len() && chars[i + 1] == '{' {
            let start = i + 2;
            let mut end = start;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            if end > start && end + 1 < chars.len() && chars[end] == '}' && chars[end + 1] == '}' {
                let name: String = chars[start..end].iter().collect();
                match map.get(name.as_str()) {
                    Some(value) => out.push_str(value),
                    None => log::warn!("prompt placeholder não fornecido, preenchido com string vazia: {name}"),
                }
                i = end + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A única prova de que `include_str!` de fato achou os 4 arquivos: se algum sumisse, o
    /// `cargo build` já teria falhado antes deste teste rodar — mas um `.md` presente e EM BRANCO
    /// compilaria normalmente e só apareceria aqui.
    #[test]
    fn every_embedded_prompt_is_found_and_non_empty() {
        for (name, content) in [
            ("daily-recap.md", DAILY_RECAP),
            ("social-post.md", SOCIAL_POST),
            ("collaboration-opening.md", COLLABORATION_OPENING),
            ("collaboration-turn.md", COLLABORATION_TURN),
        ] {
            assert!(!content.trim().is_empty(), "{name} embutido vazio — mesma classe de defeito que zera o request ao provedor");
        }
    }

    #[test]
    fn render_substitutes_every_placeholder_daily_recap_declares() {
        let rendered = render(DAILY_RECAP, &[("context", "Usuário: oi\nMika: oi!")]);
        assert!(rendered.contains("Usuário: oi\nMika: oi!"));
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn render_substitutes_every_placeholder_social_post_declares() {
        let rendered = render(SOCIAL_POST, &[("eventDescription", "1000 seguidores")]);
        assert!(rendered.contains("1000 seguidores"));
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn render_substitutes_every_placeholder_collaboration_opening_declares() {
        let rendered = render(COLLABORATION_OPENING, &[("agentA", "Mika"), ("agentB", "Luna"), ("topic", "RPG")]);
        assert!(rendered.contains("Mika") && rendered.contains("Luna") && rendered.contains("RPG"));
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn render_substitutes_every_placeholder_collaboration_turn_declares() {
        let rendered = render(COLLABORATION_TURN, &[("speaker", "Luna"), ("listener", "Mika"), ("topic", "RPG")]);
        assert!(rendered.contains("Luna") && rendered.contains("Mika") && rendered.contains("RPG"));
        assert!(!rendered.contains("{{"));
    }

    #[test]
    fn render_leaves_an_unknown_placeholder_as_empty_string_not_a_panic() {
        assert_eq!(render("oi {{nome}}", &[]), "oi ");
    }
}
