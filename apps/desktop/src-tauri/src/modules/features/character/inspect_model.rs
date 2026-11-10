//! Porte de `CharacterService.inspectLive2DModel`/`readModelFile`
//! (`apps/desktop/src/features/character/services/character.service.ts`).
//!
//! **A lacuna nomeada em `mod.rs` ("precisaria de um parser Live2D/VRM, nenhuma crate declarada")
//! não se sustenta — medida nesta rodada.** `.model3.json` é JSON puro:
//! `model_inventory::live2d_inventory` só lê campos de um `Value` que `serde_json` (já dependência
//! do crate) entrega. `.vrm` é glTF BINÁRIO, mas só o CABEÇALHO importa aqui — 12 bytes fixos +
//! 8 bytes de comprimento/tipo do primeiro chunk; nenhuma malha, textura ou skinning é lida, os
//! MESMOS bytes que `readGltfJsonChunk` (TS) já lia à mão, sem nenhuma lib de glTF do lado
//! TypeScript também. Nenhuma crate nova foi necessária — nem para o Live2D nem para o VRM.
//!
//! **Resolução de asset em build EMPACOTADO.** `readModelFile` (TS) tentava 11
//! diretórios candidatos a partir de `process.cwd()`/`__dirname`, porque rodava dentro do processo
//! principal do Electron com FS livre sobre qualquer caminho do pacote. `apps/desktop/src-tauri/
//! tauri.conf.json::bundle.resources` declara `prompts/` e `renderer/vrm/`, então o VRM oficial fica
//! disponível ao processo Rust como recurso empacotado. Os demais ativos Live2D continuam no fluxo
//! próprio do renderer. [`candidate_base_dirs`] resolve em DEV, em `cargo test` e em builds
//! empacotados usando `BaseDirectory::Resource`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

use super::model_inventory::{live2d_inventory, vrm_inventory, ModelInventory, EXPRESSIONS};

/// `DEFAULT_MODEL_PATH`/fallback de `inspectLive2DModel` — mesmos 3 candidatos do TS, na mesma
/// ordem: `modelPath` explícito, senão `live2d/<modelName>/<modelName>.model3.json` quando
/// `modelName` não é o default, senão o Live2D de demonstração.
fn resolve_relative_path(model_name: &str, model_path: &str) -> String {
    // `vrm/mika/mika.vrm#look=haru` — a skin viaja na mesma chave; o disco não conhece o fragmento.
    let model_path = model_path.split('#').next().unwrap_or_default();
    if !model_path.is_empty() {
        return model_path.to_string();
    }
    if model_name != "default" {
        return format!("live2d/{model_name}/{model_name}.model3.json");
    }
    "live2d/haru/haru_greeter_t05.model3.json".to_string()
}

/// Porte de `possibleBaseDirs`, reduzido aos dois candidatos que fazem sentido fora do processo
/// principal do Electron — ver o cabeçalho deste arquivo para o que fica de fora (build empacotado).
fn candidate_base_dirs(app: Option<&AppHandle>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(app) = app {
        if let Ok(path) = app.path().resolve("renderer", BaseDirectory::Resource) {
            if path.is_dir() {
                dirs.push(path);
            }
        }
    }
    if let Some(manifest_parent) = Path::new(env!("CARGO_MANIFEST_DIR")).parent() {
        let out_renderer = manifest_parent.join("out").join("renderer");
        if out_renderer.is_dir() {
            dirs.push(out_renderer);
        }
        let src_public = manifest_parent.join("src").join("renderer").join("public");
        if src_public.is_dir() {
            dirs.push(src_public);
        }
    }
    dirs
}

/// Porte de `readModelFile` — MESMA guarda de path traversal do TS: `character.modelPath` é
/// configurável pelo usuário (settings UI), e sem esta checagem um valor tipo
/// "../../../../etc/passwd" faria o processo ler qualquer arquivo fora da pasta de assets.
/// `canonicalize()` resolve `..`/symlinks ANTES da comparação — mais estrito que o `startsWith` de
/// string puro que o TS usa, e falha fechado (arquivo inexistente vira "não encontrado", nunca
/// escapa a checagem por não existir ainda).
fn read_model_file(relative_path: &str, base_dirs: &[PathBuf]) -> Result<(Vec<u8>, String), String> {
    let mut last_error: Option<String> = None;
    for base in base_dirs {
        let Ok(resolved_base) = base.canonicalize() else { continue };
        let joined = resolved_base.join(relative_path);
        let full_path = match joined.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                last_error = Some(format!("{relative_path}: {error}"));
                continue;
            }
        };
        if !full_path.starts_with(&resolved_base) {
            last_error = Some(format!("Path traversal recusado: \"{relative_path}\" resolve fora de {}", resolved_base.display()));
            continue;
        }
        match std::fs::read(&full_path) {
            Ok(content) if !content.is_empty() => return Ok((content, full_path.display().to_string())),
            Ok(_) => last_error = Some(format!("arquivo vazio em {}", full_path.display())),
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    Err(last_error.unwrap_or_else(|| format!("Nenhum diretório de assets encontrado para {relative_path}")))
}

fn to_json(relative_path: &str, full_path: &str, inventory: ModelInventory) -> Value {
    json!({
        "kind": inventory.kind,
        "modelPath": relative_path,
        "fullPath": full_path,
        "expressions": inventory.expressions,
        "motions": inventory.motions,
        "supportedExpressions": EXPRESSIONS,
    })
}

/// Núcleo puro (sem `AppHandle`) — testável com um diretório temporário como único `base_dir`.
fn inspect_model_in(model_name: &str, model_path: &str, base_dirs: &[PathBuf]) -> Result<Value, String> {
    let relative_path = resolve_relative_path(model_name, model_path);
    let (content, full_path) = read_model_file(&relative_path, base_dirs)?;

    let inventory = if relative_path.to_lowercase().ends_with(".vrm") {
        vrm_inventory(&content)?
    } else {
        let parsed: Value = serde_json::from_slice(&content).map_err(|error| format!("Failed to parse model file: {error}"))?;
        live2d_inventory(&parsed)
    };

    Ok(to_json(&relative_path, &full_path, inventory))
}

/// `CharacterService.inspectLive2DModel` — a fachada de produção. `app` é opcional: sem ele (hoje,
/// nenhum chamador de produção tem um `AppHandle` em mãos neste ponto — ver o cabeçalho de
/// `tool.rs`) a resolução ainda funciona em DEV/`cargo test` via `CARGO_MANIFEST_DIR`.
pub fn inspect_model(model_name: &str, model_path: &str, app: Option<&AppHandle>) -> Result<Value, String> {
    inspect_model_in(model_name, model_path, &candidate_base_dirs(app))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Dir(PathBuf);

    impl Dir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("mika-inspect-model-test-{tag}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path).unwrap();
            Dir(path)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_file(dir: &Path, rel: &str, content: &[u8]) {
        let target = dir.join(rel);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, content).unwrap();
    }

    #[test]
    fn resolve_relative_path_prefers_an_explicit_model_path_and_strips_the_skin_fragment() {
        assert_eq!(resolve_relative_path("default", "vrm/mika/mika.vrm#look=haru"), "vrm/mika/mika.vrm");
    }

    #[test]
    fn resolve_relative_path_falls_back_to_the_named_model_when_no_path_is_set() {
        assert_eq!(resolve_relative_path("haru", ""), "live2d/haru/haru.model3.json");
    }

    #[test]
    fn resolve_relative_path_falls_back_to_the_demo_live2d_when_everything_is_default() {
        assert_eq!(resolve_relative_path("default", ""), "live2d/haru/haru_greeter_t05.model3.json");
    }

    #[test]
    fn inspect_model_in_reads_a_live2d_json_fixture_from_a_temp_dir() {
        let dir = Dir::new("live2d-json");
        write_file(
            dir.0.as_path(),
            "live2d/haru/haru_greeter_t05.model3.json",
            br#"{"FileReferences":{"Expressions":[{"Name":"F01","File":"f01.exp3.json"}],"Motions":{"Idle":[{"File":"idle.motion3.json"}]}}}"#,
        );

        let result = inspect_model_in("default", "", &[dir.0.clone()]).unwrap();
        assert_eq!(result["kind"], json!("live2d"));
        assert_eq!(result["modelPath"], json!("live2d/haru/haru_greeter_t05.model3.json"));
        assert_eq!(result["expressions"][0]["name"], json!("F01"));
        assert_eq!(result["motions"][0]["name"], json!("Idle:idle"));
        assert_eq!(result["supportedExpressions"].as_array().unwrap().len(), EXPRESSIONS.len());
    }

    #[test]
    fn inspect_model_in_reads_a_vrm_binary_fixture_from_a_temp_dir() {
        let dir = Dir::new("vrm-binary");
        let json_chunk = json!({
            "extensions": { "VRMC_vrm": { "expressions": { "preset": { "happy": {} }, "custom": {} } } }
        })
        .to_string();
        let json_bytes = json_chunk.as_bytes();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x4654_6c67u32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&(20 + json_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4e4f_534au32.to_le_bytes());
        bytes.extend_from_slice(json_bytes);
        write_file(dir.0.as_path(), "vrm/mika/mika.vrm", &bytes);

        let result = inspect_model_in("default", "vrm/mika/mika.vrm", &[dir.0.clone()]).unwrap();
        assert_eq!(result["kind"], json!("vrm"));
        let names: Vec<&str> = result["expressions"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"happy"));
        assert_eq!(result["motions"].as_array().unwrap().len(), super::super::model_inventory::VRM_BODY_ACTIONS.len());
    }

    /// NEGATIVO — a mesma guarda de path traversal do TS: um `modelPath` malicioso não escapa da
    /// pasta de assets, mesmo apontando para um arquivo real fora dela.
    #[test]
    fn inspect_model_in_refuses_a_path_that_escapes_the_asset_directory() {
        let dir = Dir::new("traversal");
        // Arquivo real FORA da pasta de assets, que o traversal tentaria alcançar.
        let outside = Dir::new("traversal-outside");
        write_file(outside.0.as_path(), "secret.model3.json", b"{}");
        fs::create_dir_all(dir.0.join("live2d")).unwrap();

        let relative = format!("../{}/secret.model3.json", outside.0.file_name().unwrap().to_str().unwrap());
        let result = inspect_model_in("default", &relative, &[dir.0.clone()]);
        assert!(result.is_err(), "path traversal devia ser recusado, não lido");
    }

    #[test]
    fn inspect_model_in_fails_named_when_no_base_dir_has_the_file() {
        let error = inspect_model_in("default", "vrm/does-not-exist.vrm", &[]).unwrap_err();
        assert!(!error.is_empty());
    }

    // -- fixtures reais do repositório (zero mocks) ---------------------------------------------
    // `apps/desktop/src/renderer/public` é a MESMA árvore que `candidate_base_dirs` resolve via
    // `CARGO_MANIFEST_DIR` em dev/teste — os dois testes abaixo provam o caminho de produção
    // ponta a ponta, contra os ativos reais do avatar, não uma cópia sintética.

    #[test]
    fn inspect_model_reads_the_real_default_live2d_fixture() {
        let result = inspect_model("default", "", None).expect("fixture live2d real do repositório");
        assert_eq!(result["kind"], json!("live2d"));
        assert!(result["expressions"].as_array().unwrap().is_empty(), "haru_greeter_t05 não declara Expressions");
        let motions = result["motions"].as_array().unwrap();
        assert_eq!(motions.len(), 42, "27 motions do grupo '' (idle + m01..m26) + 15 do grupo 'Action'");
        let names: Vec<&str> = motions.iter().map(|m| m["name"].as_str().unwrap()).collect();
        assert!(names.contains(&":haru_g_idle"));
        assert!(names.contains(&"Action:mika_happy"));
    }

    #[test]
    fn inspect_model_reads_the_real_default_vrm_fixture() {
        let result = inspect_model("default", "vrm/mika/mika.vrm", None).expect("fixture vrm real do repositório");
        assert_eq!(result["kind"], json!("vrm"));
        let names: Vec<&str> = result["expressions"].as_array().unwrap().iter().map(|e| e["name"].as_str().unwrap()).collect();
        assert_eq!(names.len(), 18, "presets VRMC_vrm do mika.vrm");
        assert!(names.contains(&"happy") && names.contains(&"neutral"));
        assert_eq!(result["motions"].as_array().unwrap().len(), super::super::model_inventory::VRM_BODY_ACTIONS.len());
    }
}
