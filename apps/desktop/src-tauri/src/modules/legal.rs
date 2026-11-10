//! `legal:getDocument` — a tela Sobre/Licenças lê aqui o EULA, a política de privacidade, os avisos
//! de terceiros, o inventário de assets e a licença, **do que foi empacotado** (`bundle.resources` →
//! `legal/`, ver `tauri.conf.json`), não de uma cópia que o renderer carregue por conta própria: o
//! texto que o usuário lê tem de ser o que o instalador distribui.
//!
//! O id do documento é um enum fechado (espelho de `LEGAL_DOCUMENT_IDS` em
//! `packages/sdk/src/contracts/desktop-ipc.ts`) e o arquivo é resolvido aqui — o payload nunca
//! carrega caminho. Documento ausente é erro NOMEADO, nunca texto vazio: uma tela de EULA em branco
//! parece "sem termos", que é o contrário da verdade.
//!
//! Fora do pacote (dev, `cargo test`) cai nos arquivos da raiz do repositório, que são a fonte dos
//! mesmos recursos.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use crate::native::NativeModule;
use crate::protocol::InvokeContext;
use crate::shell_actions::is_owner_surface;

const CHANNEL: &str = "legal:getDocument";

/// `(id, arquivo)`. O arquivo está na raiz do repo e em `legal/` no pacote.
const DOCUMENTS: [(&str, &str); 5] = [
    ("eula", "EULA.md"),
    ("privacy", "PRIVACY.md"),
    ("notices", "THIRD-PARTY-NOTICES.md"),
    ("assets", "LICENSE-ASSETS.md"),
    ("license", "LICENSE"),
];

/// Os documentos que o usuário ACEITA (os demais são só consulta): a versão do aceite é o hash
/// destes dois, na ordem.
const ACCEPTED_DOCUMENTS: [&str; 2] = ["eula", "privacy"];

pub struct LegalModule;

#[async_trait]
impl NativeModule for LegalModule {
    fn name(&self) -> &'static str {
        "legal"
    }

    fn channels(&self) -> &'static [&'static str] {
        &[CHANNEL]
    }

    async fn invoke(&self, app: &AppHandle, ctx: &InvokeContext, channel: &str, payload: Value) -> Result<Value, String> {
        if channel != CHANNEL {
            return Err(format!("canal '{channel}' declarado por 'legal' mas não tratado"));
        }
        if !is_owner_surface(ctx.window_type) {
            return Err(format!("acesso negado pela política de superfície ({CHANNEL})"));
        }
        let (id, file) = parse_document(&payload)?;
        document_answer(&|file| read_document(app, file), id, file)
    }
}

/// Só EULA e política carregam a versão do ACEITE (hash dos dois, por isso precisam dos dois). Os
/// documentos de consulta (licença, avisos, assets) não participam do aceite: a `version` deles é o
/// hash do próprio texto — o contrato exige um valor, e nenhum consumidor a compara com
/// `legal.acceptedVersion` — e a leitura não toca em EULA/PRIVACY.
fn document_answer(read: &dyn Fn(&str) -> Result<String, String>, id: &str, file: &str) -> Result<Value, String> {
    let text = read(file)?;
    let version = if ACCEPTED_DOCUMENTS.contains(&id) {
        let [eula, privacy] = ACCEPTED_DOCUMENTS.map(|doc| read(document_file(doc)));
        acceptance_version(&eula?, &privacy?)
    } else {
        acceptance_version(&text, "")
    };
    Ok(json!({ "doc": id, "text": text, "version": version }))
}

fn document_file(id: &str) -> &'static str {
    DOCUMENTS.iter().find(|(doc, _)| *doc == id).map(|(_, file)| *file).expect("ACCEPTED_DOCUMENTS ⊂ DOCUMENTS")
}

fn read_document(app: &AppHandle, file: &str) -> Result<String, String> {
    read_first(&candidate_paths(app, file), file)
}

/// Hash do CONTEÚDO (16 hex de SHA-256), não um número mantido à mão: editar um documento pede novo
/// aceite sem ninguém lembrar de subir uma constante. CRLF vira LF antes de hashear para que o mesmo
/// texto com fim de linha de Windows não pareça outra versão. O separador `\0` impede que mover um
/// byte de um documento para o outro dê o mesmo hash.
pub fn acceptance_version(eula: &str, privacy: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(eula.replace("\r\n", "\n"));
    hasher.update([0]);
    hasher.update(privacy.replace("\r\n", "\n"));
    hasher.finalize().iter().take(8).map(|byte| format!("{byte:02x}")).collect()
}

pub fn modules() -> Vec<Box<dyn NativeModule>> {
    vec![Box::new(LegalModule)]
}

fn parse_document(payload: &Value) -> Result<(&'static str, &'static str), String> {
    let requested = payload.get("doc").and_then(Value::as_str).ok_or("payload sem 'doc' textual")?;
    DOCUMENTS
        .iter()
        .find(|(id, _)| *id == requested)
        .copied()
        .ok_or_else(|| format!("documento desconhecido: '{requested}'"))
}

/// O recurso empacotado primeiro; depois a raiz do repositório (dev/testes).
fn candidate_paths(app: &AppHandle, file: &str) -> Vec<PathBuf> {
    let packaged = app.path().resolve(format!("legal/{file}"), BaseDirectory::Resource).ok();
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").join(file);
    packaged.into_iter().chain(std::iter::once(repo_root)).collect()
}

fn read_first(candidates: &[PathBuf], file: &str) -> Result<String, String> {
    candidates
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())
        .ok_or_else(|| format!("documento legal '{file}' ausente do pacote"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mika-legal-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn only_listed_ids_resolve_to_a_file() {
        assert_eq!(parse_document(&json!({ "doc": "eula" })).unwrap(), ("eula", "EULA.md"));
        assert_eq!(parse_document(&json!({ "doc": "license" })).unwrap().1, "LICENSE");
        for hostile in ["../Cargo.toml", "EULA.md", "", "EULA"] {
            assert!(parse_document(&json!({ "doc": hostile })).is_err(), "{hostile} não pode virar caminho");
        }
        assert!(parse_document(&json!({})).is_err());
        assert!(parse_document(&json!({ "doc": 7 })).is_err());
    }

    fn reader<'a>(missing: &'a [&'a str]) -> impl Fn(&str) -> Result<String, String> + 'a {
        move |file| if missing.contains(&file) { Err(format!("documento legal '{file}' ausente do pacote")) } else { Ok(format!("texto de {file}")) }
    }

    /// O defeito: toda leitura calculava a versão do aceite lendo EULA E PRIVACY — um PRIVACY.md faltando
    /// derrubava até a tela de licença, que não tem nada a ver com o aceite.
    #[test]
    fn a_consultation_document_does_not_depend_on_the_accepted_documents() {
        for (id, file) in [("license", "LICENSE"), ("notices", "THIRD-PARTY-NOTICES.md"), ("assets", "LICENSE-ASSETS.md")] {
            let answer = document_answer(&reader(&["EULA.md", "PRIVACY.md"]), id, file).unwrap();
            assert_eq!(answer["text"], json!(format!("texto de {file}")));
            assert!(!answer["version"].as_str().unwrap().is_empty());
        }
    }

    /// Os documentos que o usuário aceita continuam exigindo os DOIS: a versão do aceite cobre ambos.
    #[test]
    fn an_accepted_document_still_needs_both_to_carry_the_acceptance_version() {
        let ok = document_answer(&reader(&[]), "eula", "EULA.md").unwrap();
        assert_eq!(ok["version"], json!(acceptance_version("texto de EULA.md", "texto de PRIVACY.md")));
        assert_eq!(document_answer(&reader(&[]), "privacy", "PRIVACY.md").unwrap()["version"], ok["version"]);
        let err = document_answer(&reader(&["PRIVACY.md"]), "eula", "EULA.md").unwrap_err();
        assert!(err.contains("PRIVACY.md"), "{err}");
    }

    #[test]
    fn the_packaged_copy_wins_over_the_repo_copy() {
        let dir = temp_dir("order");
        let packaged = dir.join("packaged.md");
        let repo = dir.join("repo.md");
        std::fs::write(&packaged, "empacotado").unwrap();
        std::fs::write(&repo, "repositório").unwrap();
        assert_eq!(read_first(&[packaged, repo], "x").unwrap(), "empacotado");
    }

    #[test]
    fn a_missing_document_is_a_named_error_never_empty_text() {
        let err = read_first(&[PathBuf::from("/nao/existe/EULA.md")], "EULA.md").unwrap_err();
        assert!(err.contains("EULA.md"), "{err}");
    }

    /// A versão do aceite é do CONTEÚDO que o usuário leu, não de um número que alguém lembra de
    /// subir: editar o EULA ou a política tem de pedir aceite de novo, e um checkout com CRLF
    /// (Windows/autocrlf) não pode fazer o mesmo texto parecer outra versão. O valor dourado também
    /// é o que `e2e/fixtures/legal-accepted.ts` reimplementa para pré-aceitar nos fixtures.
    #[test]
    fn acceptance_version_follows_content_not_line_endings() {
        assert_eq!(acceptance_version("termos", "privacidade"), "2e869b7c944d937b");
        assert_eq!(acceptance_version("a\r\nb", "c\r\n"), acceptance_version("a\nb", "c\n"));
        assert_ne!(acceptance_version("termos", "privacidade"), acceptance_version("termos!", "privacidade"));
        assert_ne!(acceptance_version("termos", "privacidade"), acceptance_version("termos", "privacidade!"));
        // A fronteira entre os dois textos conta: mover um byte de um para o outro muda a versão.
        assert_ne!(acceptance_version("ab", "c"), acceptance_version("a", "bc"));
    }

    #[test]
    fn the_shipped_documents_have_an_acceptance_version() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let eula = std::fs::read_to_string(root.join("EULA.md")).unwrap();
        let privacy = std::fs::read_to_string(root.join("PRIVACY.md")).unwrap();
        let version = acceptance_version(&eula, &privacy);
        assert_eq!(version.len(), 16, "{version}");
        assert!(version.chars().all(|c| c.is_ascii_hexdigit()), "{version}");
    }

    /// Contrato cross-language: o enum do contrato TypeScript, o canal e a lista de recursos do
    /// `tauri.conf.json` precisam concordar com `DOCUMENTS` — qualquer um divergindo vira tela de
    /// termos que falha só no instalador.
    #[test]
    fn contract_ids_and_bundle_resources_match_the_documents_table() {
        let contract = include_str!("../../../../../packages/sdk/src/contracts/desktop-ipc.ts");
        let tauri_conf = include_str!("../../tauri.conf.json");
        assert!(contract.contains(&format!("channel: '{CHANNEL}'")), "canal fora do contrato TypeScript");
        assert!(
            contract.contains("responseSchema: z.object({ doc: z.enum(LEGAL_DOCUMENT_IDS), text: z.string(), version: z.string().min(1) })"),
            "a resposta do contrato TypeScript não carrega a versão do aceite"
        );
        let ids_line = contract.lines().find(|line| line.contains("LEGAL_DOCUMENT_IDS = [")).expect("LEGAL_DOCUMENT_IDS");
        for (id, file) in DOCUMENTS {
            assert!(ids_line.contains(&format!("'{id}'")), "id '{id}' ausente de LEGAL_DOCUMENT_IDS");
            assert!(tauri_conf.contains(&format!("\"legal/{file}\"")), "{file} não está em bundle.resources");
            assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").join(file).is_file(), "{file} ausente da raiz");
        }
        assert_eq!(ids_line.matches('\'').count(), DOCUMENTS.len() * 2, "ids a mais ou a menos que a tabela");
    }
}
