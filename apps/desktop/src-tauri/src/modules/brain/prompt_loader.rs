//! Carregamento dos `.md` de `services/brain/prompts/` — mesmo padrão fail-loud de
//! `chat.rs::load_prompt_in`, duplicado deste (função lá não é `pub`, e `chat.rs` está fora do
//! escopo desta rodada), mas compartilhado DENTRO de `brain` por [`super::reflection`] (prompts
//! `reflection-system`/`reflection-subconscious`) e [`super::post_turn::facts`] (prompt
//! `fact-extraction`) — os três vivem no MESMO diretório TypeScript (`services/brain/prompts/*.md`,
//! namespace `brain`), então um loader só evita duas cópias divergindo (mesmo raciocínio que juntou
//! a extração de JSON em [`super::response_json`]).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

pub(super) const PROMPT_NAMESPACE: &str = "brain";

fn prompts_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(path) = app.path().resolve("prompts", BaseDirectory::Resource) {
        if path.is_dir() {
            return Some(path);
        }
    }

    let dev_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.join("out").join("prompts");
    dev_root.is_dir().then_some(dev_root)
}

fn override_root(app: &AppHandle) -> Option<PathBuf> {
    crate::state::app_data_dir(app).ok().map(|dir| dir.join("prompt-overrides"))
}

/// Preenche `{{placeholder}}` (`\w+` ASCII), mesmo contrato de `renderTemplate` em
/// `packages/sdk/src/prompts/loader.ts` e de `chat.rs::render_template`.
fn render_template(template: &str, vars: &HashMap<String, String>) -> (String, Vec<String>) {
    let chars: Vec<char> = template.chars().collect();
    let mut out = String::with_capacity(template.len());
    let mut missing = Vec::new();
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
                match vars.get(&name) {
                    Some(value) => out.push_str(value),
                    None => {
                        if !missing.contains(&name) {
                            missing.push(name);
                        }
                    }
                }
                i = end + 2;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }

    (out, missing)
}

/// Primeiro arquivo NÃO VAZIO entre os candidatos (override antes do default). Um `.md` presente
/// mas em branco conta como ausente — o mesmo incidente que `chat.rs`/`health.rs` documentam: um
/// prompt vazio virou request de zero token, 400, e o provedor foi desativado.
fn read_first_non_empty(candidates: &[PathBuf], key: &str) -> Option<String> {
    for dir in candidates {
        if let Ok(content) = fs::read_to_string(dir.join(key)) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Núcleo puro de [`load_prompt`] — sem `AppHandle`, testável com diretórios temporários. FALHA
/// ALTO (`Err`) quando o prompt não existe em nenhum candidato: nunca devolve string vazia.
fn load_prompt_in(
    root: &Path,
    override_base: Option<&Path>,
    namespace: &str,
    key: &str,
    vars: &HashMap<String, String>,
) -> Result<String, String> {
    let key = if key.ends_with(".md") { key.to_string() } else { format!("{key}.md") };

    let mut candidates = Vec::new();
    if let Some(base) = override_base {
        candidates.push(base.join(namespace));
    }
    candidates.push(root.join(namespace));

    let template = read_first_non_empty(&candidates, &key).ok_or_else(|| {
        format!("Prompt não encontrado: {namespace}/{key} (procurado em {} pasta(s))", candidates.len())
    })?;

    if vars.is_empty() {
        return Ok(template);
    }

    let (rendered, missing) = render_template(&template, vars);
    for placeholder in &missing {
        log::warn!("prompt placeholder não fornecido, preenchido com string vazia: {namespace}/{key}:{placeholder}");
    }
    Ok(rendered)
}

/// `load_prompt(app, "reflection-system", vars)`/`load_prompt(app, "fact-extraction", vars)` etc.
/// — a fachada com `AppHandle` de verdade, sempre no namespace `brain`.
pub(super) fn load_prompt(app: &AppHandle, key: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    load_prompt_namespaced(app, PROMPT_NAMESPACE, key, vars)
}

/// Mesma fachada, namespace explícito — `post_turn::pipeline` usa isto com `"memory"` para
/// `consolidation.md` (`services/memory/prompts/`, diretório IRMÃO de `services/brain/prompts/`,
/// não um arquivo a mais dentro dele — confirmado em `apps/desktop/out/prompts/memory/`).
pub(super) fn load_prompt_namespaced(app: &AppHandle, namespace: &str, key: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    let root = prompts_root(app).ok_or_else(|| "Pasta de prompts não encontrada no pacote".to_string())?;
    load_prompt_in(&root, override_root(app).as_deref(), namespace, key, vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    // --- render_template ----------------------------------------------------------------------

    #[test]
    fn render_template_substitui_conversation_text() {
        let (out, missing) = render_template("Analise: {{conversationText}}", &vars(&[("conversationText", "Usuário: oi\nMika: olá")]));
        assert_eq!(out, "Analise: Usuário: oi\nMika: olá");
        assert!(missing.is_empty());
    }

    #[test]
    fn render_template_reporta_placeholder_ausente_sem_falhar() {
        let (out, missing) = render_template("{{conversationText}} fim", &HashMap::new());
        assert_eq!(out, " fim");
        assert_eq!(missing, vec!["conversationText".to_string()]);
    }

    // --- load_prompt_in: fail-loud ------------------------------------------------------------

    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-brain-prompt-test-{tag}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Dir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_file(dir: &Path, rel: &str, content: &str) {
        let target = dir.join(rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, content).unwrap();
    }

    #[test]
    fn load_prompt_in_falha_alto_quando_o_prompt_nao_existe() {
        let root = Dir::new("missing");
        let result = load_prompt_in(root.path(), None, "brain", "reflection-system", &HashMap::new());
        assert!(result.is_err());
    }

    /// Mesmo incidente documentado no cabeçalho: `.md` presente mas em branco NUNCA vira `Ok("")`.
    #[test]
    fn load_prompt_in_trata_arquivo_em_branco_como_ausente() {
        let root = Dir::new("blank");
        write_file(root.path(), "brain/reflection-system.md", "   \n  ");

        let result = load_prompt_in(root.path(), None, "brain", "reflection-system", &HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn load_prompt_in_carrega_e_renderiza_o_prompt_de_reflexao() {
        let root = Dir::new("ok");
        write_file(root.path(), "brain/reflection-subconscious.md", "Reflita sobre: {{conversationText}}");

        let result = load_prompt_in(
            root.path(),
            None,
            "brain",
            "reflection-subconscious",
            &vars(&[("conversationText", "Usuário: oi\nMika: olá")]),
        )
        .unwrap();
        assert_eq!(result, "Reflita sobre: Usuário: oi\nMika: olá");
    }

    #[test]
    fn load_prompt_in_carrega_o_prompt_de_extracao_de_fatos() {
        let root = Dir::new("facts");
        write_file(root.path(), "brain/fact-extraction.md", "Extraia fatos.");

        let result = load_prompt_in(root.path(), None, "brain", "fact-extraction", &HashMap::new()).unwrap();
        assert_eq!(result, "Extraia fatos.");
    }

    #[test]
    fn load_prompt_in_prefere_o_override_sobre_o_default() {
        let root = Dir::new("override-root");
        write_file(root.path(), "brain/reflection-system.md", "default");
        let override_dir = Dir::new("override-base");
        write_file(override_dir.path(), "brain/reflection-system.md", "sobrescrito");

        let result = load_prompt_in(root.path(), Some(override_dir.path()), "brain", "reflection-system", &HashMap::new()).unwrap();
        assert_eq!(result, "sobrescrito");
    }
}
