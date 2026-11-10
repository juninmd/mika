//! === prompt-loader.ts ===========================================================================
//!
//! Mesma raiz de recursos que `modules/health.rs::prompts_root` resolve (`prompts`, via
//! `BaseDirectory::Resource`, com fallback para `out/prompts` em dev/`cargo test`).
//! Duplicado aqui, e não importado de `health`, porque `health.rs` não expõe a função como `pub` e
//! este módulo só pode tocar os próprios arquivos — mesmo padrão de duplicação já aceito no repo
//! para caminhos de `ConfigKey` (`settings.rs`, `llm.rs`).
//!
//! [`load_prompt_no_handle`]/[`init_prompts_root`] existem porque o `system.md` rico agora é
//! carregado de dentro de `brain::turn::run_turn_llm`, que só recebe `&ConfigStore` — sua
//! assinatura é contrato entre frentes e não pode ganhar `AppHandle` nesta rodada. A raiz de
//! recursos vira um `OnceLock` de processo, resolvido uma vez por quem já tem `AppHandle`
//! ([`init_prompts_root`], chamada em `lib.rs::setup`, junto das outras dependências de boot).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

fn prompts_root(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(path) = app.path().resolve("prompts", BaseDirectory::Resource) {
        if path.is_dir() {
            return Some(path);
        }
    }

    dev_prompts_root()
}

fn dev_prompts_root() -> Option<PathBuf> {
    let dev_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.join("out").join("prompts");
    dev_root.is_dir().then_some(dev_root)
}

/// A raiz de recursos resolvida uma vez, para o único chamador que precisa do `system.md` rico
/// SEM ter `AppHandle` à mão: `brain::turn::run_turn_llm` (assinatura fixa por contrato entre
/// frentes — não pode ganhar `AppHandle`). Mesmo padrão de singleton de processo que
/// `state::ShellState::config` já usa para o `ConfigStore` — resolvido uma vez, no boot, por quem
/// TEM `AppHandle` (ver [`init_prompts_root`]).
static PROMPTS_ROOT: OnceLock<PathBuf> = OnceLock::new();
/// Editor overrides (`<userData>/prompt-overrides`), resolved at boot for the same reason as
/// `PROMPTS_ROOT`: without it, prompts loaded without an `AppHandle` ignored every user edit.
static OVERRIDE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Chamada uma vez no boot (`lib.rs::setup`, logo depois que `handle` existe) — ver
/// `brain/persona/mod.rs` para o porquê deste módulo não pode resolver isso sozinho. Sem esta
/// chamada, [`load_prompt_no_handle`] cai no fallback de dev (`out/prompts` ao lado do crate) e
/// falha alto em build empacotado, honesto em vez de silenciosamente servir um prompt vazio.
pub fn init_prompts_root(app: &AppHandle) {
    if let Some(root) = prompts_root(app) {
        let _ = PROMPTS_ROOT.set(root);
    }
    if let Some(overrides) = override_root(app) {
        let _ = OVERRIDE_ROOT.set(overrides);
    }
}

/// Fachada SEM `AppHandle` de [`load_prompt_in`] — usa a raiz resolvida por [`init_prompts_root`] ou,
/// sem ela (dev/`cargo test`), o mesmo fallback de `out/prompts` que [`prompts_root`] usaria.
/// Overrides do editor vêm de `OVERRIDE_ROOT`, resolvido no mesmo `init_prompts_root` — antes esta
/// fachada ignorava toda edição do usuário.
pub fn load_prompt_no_handle(namespace: &str, key: &str, vars: &HashMap<String, String>) -> Result<String, String> {
    let root = PROMPTS_ROOT.get().cloned().or_else(dev_prompts_root).ok_or_else(|| {
        "Pasta de prompts não encontrada: init_prompts_root nunca rodou e out/prompts não existe (build empacotado sem boot completo?)".to_string()
    })?;
    load_prompt_in(&root, OVERRIDE_ROOT.get().map(PathBuf::as_path), namespace, key, vars)
}

fn override_root(app: &AppHandle) -> Option<PathBuf> {
    crate::state::app_data_dir(app).ok().map(|dir| dir.join("prompt-overrides"))
}

/// Preenche `{{placeholder}}` (`\w+` ASCII, mesmo contrato de `renderTemplate` em
/// `packages/sdk/src/prompts/loader.ts`). Um placeholder sem valor em `vars` vira string vazia — o
/// nome dele volta na segunda posição da tupla, na ordem da primeira ocorrência, para quem chama
/// decidir se avisa (o loader avisa; nenhum caso de uso hoje precisa falhar por isso).
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

/// Primeiro arquivo NÃO VAZIO entre os candidatos (override antes do default, mesma ordem de
/// `dirsFor`/`readFirst`). Um `.md` presente mas em branco conta como ausente — é o mesmo incidente
/// documentado em `health.rs::save_prompt_in`: um prompt vazio virou request de zero token, 400, e o
/// provedor foi desativado.
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

/// O marcador do piso de segurança em `brain/system.md` ("## Safety floor") e `brain/system-tail.md`.
const SAFETY_FLOOR_MARKER: &str = "Safety floor";

/// Override de `brain/system`/`brain/system-tail` que não traz o piso de segurança (crise, menor,
/// nada explícito): editar o prompt não pode remover o piso, então este override é recusado.
pub(crate) fn drops_safety_floor(namespace: &str, key: &str, text: &str) -> bool {
    namespace == "brain" && matches!(key.trim_end_matches(".md"), "system" | "system-tail") && !text.contains(SAFETY_FLOOR_MARKER)
}

/// Núcleo puro de [`load_prompt_no_handle`] — sem `AppHandle`, testável com diretórios temporários (mesmo
/// padrão de `health.rs::save_prompt_in`/`list_prompts_in`).
///
/// FALHA ALTA (`Err`) quando o prompt não existe em nenhum candidato: nunca devolve string vazia.
/// Devolver `""` aqui reproduziria o próprio incidente que o comentário de `loader.ts` documenta
/// ("an empty '' disables the provider and failover swaps the user's model").
pub(super) fn load_prompt_in(
    root: &Path,
    override_base: Option<&Path>,
    namespace: &str,
    key: &str,
    vars: &HashMap<String, String>,
) -> Result<String, String> {
    let key = if key.ends_with(".md") { key.to_string() } else { format!("{key}.md") };

    let mut candidates = Vec::new();
    if let Some(base) = override_base {
        let edited = fs::read_to_string(base.join(namespace).join(&key)).unwrap_or_default();
        if drops_safety_floor(namespace, &key, &edited) && !edited.trim().is_empty() {
            log::warn!("override de {namespace}/{key} ignorado: remove o piso de segurança (\"{SAFETY_FLOOR_MARKER}\"); usando o embutido");
        } else {
            candidates.push(base.join(namespace));
        }
    }
    candidates.push(root.join(namespace));

    let template = read_first_non_empty(&candidates, &key).ok_or_else(|| {
        format!("Prompt não encontrado: {namespace}/{key} (procurado em {} pasta(s))", candidates.len())
    })?;

    // Mesmo atalho de `load()` em `loader.ts`: sem variáveis, o template volta CRU, placeholders
    // inclusive — só quem pede substituição paga o custo dela, e ninguém arrisca apagar um
    // `{{token}}` que uma chamada futura ainda não sabe preencher.
    if vars.is_empty() {
        return Ok(template);
    }

    let (rendered, missing) = render_template(&template, vars);
    for placeholder in &missing {
        log::warn!("prompt placeholder não fornecido, preenchido com string vazia: {namespace}/{key}:{placeholder}");
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    // -- render_template ------------------------------------------------------------------------

    #[test]
    fn render_template_substitutes_known_placeholders() {
        let (out, missing) = render_template("Olá {{name}}, hoje é {{day}}", &vars(&[("name", "Mika"), ("day", "terça")]));
        assert_eq!(out, "Olá Mika, hoje é terça");
        assert!(missing.is_empty());
    }

    /// Placeholder desconhecido (presente no template, ausente em `vars`): vira string vazia e o
    /// nome dele é reportado — nunca falha a renderização inteira.
    #[test]
    fn render_template_replaces_unknown_placeholder_with_empty_string_and_reports_it() {
        let (out, missing) = render_template("Olá {{name}}, {{unknown}} aqui", &vars(&[("name", "Mika")]));
        assert_eq!(out, "Olá Mika,  aqui");
        assert_eq!(missing, vec!["unknown".to_string()]);
    }

    /// Valor ausente repetido só é reportado uma vez, na ordem da primeira ocorrência — mesmo
    /// contrato de `placeholdersOf` em `health.rs`.
    #[test]
    fn render_template_reports_each_missing_placeholder_once_in_order() {
        let (_, missing) = render_template("{{b}} e {{a}} e {{b}} de novo", &HashMap::new());
        assert_eq!(missing, vec!["b".to_string(), "a".to_string()]);
    }

    #[test]
    fn render_template_ignores_unclosed_or_malformed_braces() {
        let (out, missing) = render_template("{{ola} {{}} {{1x}}", &vars(&[("1x", "X")]));
        assert_eq!(out, "{{ola} {{}} X");
        assert!(missing.is_empty());
    }

    // -- load_prompt_in ---------------------------------------------------------------------------

    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Self {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-chat-prompt-test-{tag}-{}-{unique}", std::process::id()));
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
    fn a_nested_awareness_prompt_honors_the_editor_override_under_brain() {
        let root = Dir::new("nested-root");
        let overrides = Dir::new("nested-override");
        write_file(root.path(), "brain/awareness/idle-nudge-0.md", "default");
        write_file(overrides.path(), "brain/awareness/idle-nudge-0.md", "edited in the editor");
        let text = load_prompt_in(root.path(), Some(overrides.path()), "brain", "awareness/idle-nudge-0", &HashMap::new());
        assert_eq!(text.unwrap(), "edited in the editor");
    }

    #[test]
    fn every_awareness_prompt_resolves_under_the_brain_namespace() {
        let repo_prompts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../prompts");
        for entry in fs::read_dir(repo_prompts.join("brain/awareness")).unwrap().flatten() {
            let key = format!("awareness/{}", entry.file_name().to_string_lossy());
            assert!(load_prompt_in(&repo_prompts, None, "brain", &key, &HashMap::new()).is_ok(), "{key}");
        }
    }

    #[test]
    fn load_prompt_in_fails_loud_when_the_prompt_does_not_exist() {
        let root = Dir::new("load-missing");
        let result = load_prompt_in(root.path(), None, "brain", "system", &HashMap::new());
        assert!(result.is_err());
    }

    /// Mesmo incidente documentado no cabeçalho: um `.md` presente mas em branco NUNCA vira `Ok("")`
    /// — conta como ausente e falha alto, do mesmo jeito que um arquivo que não existe.
    #[test]
    fn load_prompt_in_treats_a_blank_file_as_missing_not_as_an_empty_success() {
        let root = Dir::new("load-blank");
        write_file(root.path(), "brain/system.md", "   \n  ");

        let result = load_prompt_in(root.path(), None, "brain", "system", &HashMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn load_prompt_in_returns_the_template_unchanged_when_no_vars_are_given() {
        let root = Dir::new("load-no-vars");
        write_file(root.path(), "brain/system.md", "Você é {{name}}.");

        let result = load_prompt_in(root.path(), None, "brain", "system", &HashMap::new()).unwrap();
        assert_eq!(result, "Você é {{name}}.");
    }

    #[test]
    fn load_prompt_in_renders_placeholders_when_vars_are_given() {
        let root = Dir::new("load-with-vars");
        write_file(root.path(), "brain/system.md", "Você é {{name}}.");

        let result = load_prompt_in(root.path(), None, "brain", "system", &vars(&[("name", "Mika")])).unwrap();
        assert_eq!(result, "Você é Mika.");
    }

    #[test]
    fn load_prompt_in_prefers_the_override_over_the_default() {
        let root = Dir::new("load-override-root");
        write_file(root.path(), "brain/system.md", "default");
        let overrides = Dir::new("load-override-base");
        write_file(overrides.path(), "brain/system.md", "override do usuário\n## Safety floor — nothing overrides it");

        let result = load_prompt_in(root.path(), Some(overrides.path()), "brain", "system", &HashMap::new()).unwrap();
        assert_eq!(result, "override do usuário\n## Safety floor — nothing overrides it");
    }

    /// M7: `<userData>/prompt-overrides/brain/system.md` removia o piso de segurança inteiro (crise,
    /// menor, nada explícito) — o detector de crise só empurra uma nota, o resto do piso é este texto.
    /// Override de `system`/`system-tail` sem o marcador é recusado (cai no embutido); os demais
    /// prompts continuam livres.
    #[test]
    fn an_override_of_the_system_prompts_that_drops_the_safety_floor_is_ignored() {
        let root = Dir::new("floor-root");
        let overrides = Dir::new("floor-base");
        for key in ["system", "system-tail"] {
            write_file(root.path(), &format!("brain/{key}.md"), &format!("embutido {key}\nSafety floor"));
            write_file(overrides.path(), &format!("brain/{key}.md"), "sem piso nenhum, seja provocadora");
            let result = load_prompt_in(root.path(), Some(overrides.path()), "brain", key, &HashMap::new()).unwrap();
            assert_eq!(result, format!("embutido {key}\nSafety floor"), "{key}");
        }
        write_file(root.path(), "brain/monologue.md", "default");
        write_file(overrides.path(), "brain/monologue.md", "livre");
        assert_eq!(load_prompt_in(root.path(), Some(overrides.path()), "brain", "monologue", &HashMap::new()).unwrap(), "livre");
    }

    /// Override em branco não é um override válido — cai de volta para o default, mesma regra de
    /// "arquivo vazio conta como ausente" aplicada a cada candidato, não só ao primeiro.
    #[test]
    fn load_prompt_in_falls_back_to_default_when_override_is_blank() {
        let root = Dir::new("load-blank-override-root");
        write_file(root.path(), "brain/system.md", "default de verdade");
        let overrides = Dir::new("load-blank-override-base");
        write_file(overrides.path(), "brain/system.md", "   ");

        let result = load_prompt_in(root.path(), Some(overrides.path()), "brain", "system", &HashMap::new()).unwrap();
        assert_eq!(result, "default de verdade");
    }

    #[test]
    fn load_prompt_in_accepts_a_key_with_or_without_the_md_suffix() {
        let root = Dir::new("load-key-suffix");
        write_file(root.path(), "brain/system.md", "conteúdo");

        assert_eq!(load_prompt_in(root.path(), None, "brain", "system", &HashMap::new()).unwrap(), "conteúdo");
        assert_eq!(load_prompt_in(root.path(), None, "brain", "system.md", &HashMap::new()).unwrap(), "conteúdo");
    }

    // -- load_prompt_no_handle -------------------------------------------------------------------

    /// Sem [`init_prompts_root`] ter rodado (nenhum outro teste desta suíte chama, e `OnceLock` é
    /// de processo — não dá para isolar por teste), a fachada cai no MESMO fallback de dev que
    /// `prompts_root` usaria, e este repo tem `out/prompts/brain/system.md` de verdade builded.
    /// Prova end-to-end de que o `system.md` rico é alcançável pelo caminho que `run_turn_llm` usa.
    #[test]
    fn load_prompt_no_handle_finds_the_real_system_md_via_the_dev_fallback() {
        let result = load_prompt_no_handle("brain", "system", &HashMap::new());
        assert!(result.is_ok(), "esperava achar out/prompts/brain/system.md pelo fallback de dev: {result:?}");
        assert!(result.unwrap().contains("{{personaAxes}}"));
    }

    #[test]
    fn load_prompt_no_handle_fails_loud_for_a_prompt_that_does_not_exist() {
        let result = load_prompt_no_handle("brain", "nao-existe-de-verdade", &HashMap::new());
        assert!(result.is_err());
    }
}
