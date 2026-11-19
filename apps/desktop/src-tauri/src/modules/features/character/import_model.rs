//! "Importar modelo" da tela de skins: copia um `.vrm` ou a pasta de um `.model3.json` para
//! `<app_data>/live2d/<nome>/`, a raiz que o esquema `mika-live2d` (`live2d_protocol.rs`) já serve.
//! Nenhum modelo de terceiro vem no instalador, então é o único jeito de usar um avatar próprio.
//!
//! O arquivo vem do diálogo nativo aberto AQUI, nunca de um caminho enviado pelo renderer: um XSS
//! na tela de settings não consegue copiar um arquivo arbitrário do disco para uma pasta servida.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

/// Teto por arquivo e por pasta: um modelo VRoid fica em dezenas de MB; isto barra o disco inteiro.
const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILES: usize = 2000;
const MAX_DEPTH: usize = 8;

/// Abre o diálogo e importa o escolhido. `null` quando o usuário cancela.
pub fn import(app: &AppHandle) -> Result<Value, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("VRM / Live2D", &["vrm", "json"])
        .blocking_pick_file();
    let Some(picked) = picked else { return Ok(Value::Null) };
    let source = picked.into_path().map_err(|e| e.to_string())?;
    let root = crate::state::app_data_dir(app).map_err(|e| e.to_string())?.join("live2d");
    Ok(json!({ "modelPath": import_into(&source, &root)? }))
}

/// Núcleo testável: devolve o `character.modelPath` (`live2d/<nome>/<arquivo>`) do modelo copiado.
fn import_into(source: &Path, root: &Path) -> Result<String, String> {
    let name = source.file_name().and_then(|n| n.to_str()).ok_or("nome de arquivo inválido")?;
    let lower = name.to_lowercase();
    if lower.ends_with(".vrm") {
        import_vrm(source, root, &slug(&name[..name.len() - 4]))
    } else if lower.ends_with(".model3.json") {
        import_live2d(source, root, &slug(&name[..name.len() - 12]))
    } else {
        Err("escolha um arquivo .vrm ou .model3.json".into())
    }
}

/// Nome de pasta e de arquivo seguro para URL e para qualquer SO; o original pode ter espaço, `#`, acento.
fn slug(stem: &str) -> String {
    let mapped: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '-' })
        .collect();
    let joined = mapped.split('-').filter(|part| !part.is_empty()).collect::<Vec<_>>().join("-");
    let cut: String = joined.chars().take(64).collect();
    let trimmed = cut.trim_matches('-');
    if trimmed.is_empty() { "modelo".into() } else { trimmed.to_string() }
}

fn import_vrm(source: &Path, root: &Path, slug: &str) -> Result<String, String> {
    let size = fs::metadata(source).map_err(|e| e.to_string())?.len();
    if size > MAX_BYTES {
        return Err("modelo grande demais (limite de 512 MB)".into());
    }
    let bytes = fs::read(source).map_err(|e| e.to_string())?;
    if !bytes.starts_with(b"glTF") {
        return Err("o arquivo não é um VRM (glTF binário)".into());
    }
    let dest = fresh_dir(root, slug)?;
    fs::write(dest.join(format!("{slug}.vrm")), bytes).map_err(|e| e.to_string())?;
    Ok(format!("live2d/{slug}/{slug}.vrm"))
}

fn import_live2d(source: &Path, root: &Path, slug: &str) -> Result<String, String> {
    let manifest: Value =
        serde_json::from_slice(&fs::read(source).map_err(|e| e.to_string())?).map_err(|_| "model3.json inválido")?;
    if manifest["FileReferences"]["Moc"].as_str().is_none() {
        return Err("model3.json sem FileReferences.Moc: não é um modelo Live2D Cubism".into());
    }
    let folder = source.parent().ok_or("pasta do modelo inválida")?;
    let dest = fresh_dir(root, slug)?;
    let mut budget = (0usize, 0u64);
    if let Err(error) = copy_tree(folder, &dest, 0, &mut budget) {
        let _ = fs::remove_dir_all(&dest);
        return Err(error);
    }
    // Renomeia só o manifesto: as referências dele são relativas, o nome dele não é usado por ninguém.
    let original = dest.join(source.file_name().ok_or("nome inválido")?);
    fs::rename(original, dest.join(format!("{slug}.model3.json"))).map_err(|e| e.to_string())?;
    Ok(format!("live2d/{slug}/{slug}.model3.json"))
}

/// Reimportar o mesmo nome substitui a cópia anterior em vez de misturar arquivos das duas.
fn fresh_dir(root: &Path, slug: &str) -> Result<PathBuf, String> {
    let dest = root.join(slug);
    if dest.exists() {
        fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Cópia recursiva com teto de arquivos, bytes e profundidade; symlink é pulado, nunca seguido.
fn copy_tree(from: &Path, to: &Path, depth: usize, budget: &mut (usize, u64)) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("pasta do modelo aninhada demais".into());
    }
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            copy_tree(&entry.path(), &target, depth + 1, budget)?;
        } else if kind.is_file() {
            budget.0 += 1;
            budget.1 += entry.metadata().map_err(|e| e.to_string())?.len();
            if budget.0 > MAX_FILES || budget.1 > MAX_BYTES {
                return Err("pasta do modelo grande demais (2000 arquivos ou 512 MB)".into());
            }
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("mika-import-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &Path, rel: &str, bytes: &[u8]) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    const MANIFEST: &[u8] = br#"{"FileReferences":{"Moc":"m.moc3","Textures":["tex/t.png"]}}"#;

    #[test]
    fn a_vrm_lands_under_a_url_safe_name_the_protocol_serves() {
        let (src, root) = (Dir::new("vrm-src"), Dir::new("vrm-root"));
        write(&src.0, "Minha Skin #2.vrm", b"glTF\x02\0\0\0");
        let path = import_into(&src.0.join("Minha Skin #2.vrm"), &root.0).unwrap();
        assert_eq!(path, "live2d/Minha-Skin-2/Minha-Skin-2.vrm");
        assert_eq!(fs::read(root.0.join("Minha-Skin-2/Minha-Skin-2.vrm")).unwrap(), b"glTF\x02\0\0\0");
    }

    #[test]
    fn a_file_that_is_not_gltf_is_refused_before_anything_is_written() {
        let (src, root) = (Dir::new("fake-src"), Dir::new("fake-root"));
        write(&src.0, "fake.vrm", b"<html>");
        assert!(import_into(&src.0.join("fake.vrm"), &root.0).is_err());
        assert!(!root.0.join("fake").exists());
    }

    #[test]
    fn a_live2d_model_brings_its_textures_and_motions_along() {
        let (src, root) = (Dir::new("l2d-src"), Dir::new("l2d-root"));
        write(&src.0, "haru.model3.json", MANIFEST);
        write(&src.0, "m.moc3", b"MOC3");
        write(&src.0, "tex/t.png", b"png");
        let path = import_into(&src.0.join("haru.model3.json"), &root.0).unwrap();
        assert_eq!(path, "live2d/haru/haru.model3.json");
        assert!(root.0.join("haru/m.moc3").is_file());
        assert!(root.0.join("haru/tex/t.png").is_file());
    }

    #[test]
    fn a_json_that_is_not_a_cubism_model_is_refused() {
        let (src, root) = (Dir::new("json-src"), Dir::new("json-root"));
        write(&src.0, "x.model3.json", b"{}");
        assert!(import_into(&src.0.join("x.model3.json"), &root.0).is_err());
        assert!(import_into(&src.0.join("x.json"), &root.0).is_err(), "só .vrm ou .model3.json");
    }

    #[test]
    fn reimporting_replaces_the_old_copy_instead_of_mixing_files() {
        let (src, root) = (Dir::new("again-src"), Dir::new("again-root"));
        write(&root.0, "mika/stale.png", b"old");
        write(&src.0, "mika.vrm", b"glTF");
        import_into(&src.0.join("mika.vrm"), &root.0).unwrap();
        assert!(!root.0.join("mika/stale.png").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_inside_the_model_folder_is_not_followed() {
        let (src, root, outside) = (Dir::new("link-src"), Dir::new("link-root"), Dir::new("link-out"));
        write(&src.0, "a.model3.json", MANIFEST);
        write(&outside.0, "secret.txt", b"secret");
        std::os::unix::fs::symlink(outside.0.join("secret.txt"), src.0.join("secret.txt")).unwrap();
        import_into(&src.0.join("a.model3.json"), &root.0).unwrap();
        assert!(!root.0.join("a/secret.txt").exists());
    }

    #[test]
    fn slug_never_comes_back_empty_or_with_a_path_separator() {
        assert_eq!(slug("../.."), "modelo");
        assert_eq!(slug("ç/ã"), "modelo");
        assert_eq!(slug("Aurora_v2"), "Aurora_v2");
    }
}
