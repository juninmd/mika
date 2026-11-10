//! Parsing do glTF BINÁRIO por trás de `.vrm` — extraído de `model_inventory.rs` pelo teto de 300
//! linhas do AGENTS.md. Só o CABEÇALHO importa aqui: 12 bytes fixos + 8 bytes de comprimento/tipo
//! do primeiro chunk — os MESMOS bytes que `readGltfJsonChunk`/`vrmExpressionNames` (TS,
//! `apps/desktop/src/features/character/services/vrm-inventory.ts`) já liam à mão, sem nenhuma lib
//! de glTF do lado TypeScript também. Nenhuma malha, textura ou skinning é lida — só o chunk JSON.

use serde_json::Value;

const GLTF_MAGIC: u32 = 0x4654_6c67;
const CHUNK_JSON: u32 = 0x4e4f_534a;
const HEADER_BYTES: usize = 12;
const CHUNK_HEADER_BYTES: usize = 8;

fn read_u32_le(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes.get(offset..offset + 4)?.try_into().ok().map(u32::from_le_bytes)
}

/// Porte de `readGltfJsonChunk`. Diferente do TS (que deixa `subarray` estourar em silêncio se
/// `chunkLength` mentir sobre o tamanho do arquivo), aqui o limite é checado explicitamente — um
/// `.vrm` truncado ou corrompido (já aconteceu neste repo por falta de `binary` no
/// `.gitattributes`) vira erro nomeado, nunca um slice fora dos limites.
pub(crate) fn read_gltf_json_chunk(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() < HEADER_BYTES + CHUNK_HEADER_BYTES {
        return Err("arquivo não é glTF binário (.vrm/.glb): menor que o cabeçalho mínimo".to_string());
    }
    if read_u32_le(bytes, 0) != Some(GLTF_MAGIC) {
        return Err("arquivo não é glTF binário (.vrm/.glb)".to_string());
    }
    let chunk_length = read_u32_le(bytes, HEADER_BYTES).unwrap_or(0) as usize;
    if read_u32_le(bytes, HEADER_BYTES + 4) != Some(CHUNK_JSON) {
        return Err("primeiro chunk do glTF não é JSON".to_string());
    }

    let start = HEADER_BYTES + CHUNK_HEADER_BYTES;
    let end = start
        .checked_add(chunk_length)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| "chunk JSON do glTF excede o tamanho do arquivo".to_string())?;

    serde_json::from_slice(&bytes[start..end]).map_err(|error| format!("JSON do chunk glTF inválido: {error}"))
}

/// Porte de `vrmExpressionNames`: VRM 1.0 primeiro (`VRMC_vrm.expressions.{preset,custom}`), VRM
/// 0.x como fallback (`VRM.blendShapeMaster.blendShapeGroups`).
pub(crate) fn vrm_expression_names(scene: &Value) -> Vec<String> {
    let mut from_vrm1 = Vec::new();
    if let Some(expressions) = scene.pointer("/extensions/VRMC_vrm/expressions") {
        if let Some(preset) = expressions.get("preset").and_then(Value::as_object) {
            from_vrm1.extend(preset.keys().cloned());
        }
        if let Some(custom) = expressions.get("custom").and_then(Value::as_object) {
            from_vrm1.extend(custom.keys().cloned());
        }
    }
    if !from_vrm1.is_empty() {
        return from_vrm1;
    }

    scene
        .pointer("/extensions/VRM/blendShapeMaster/blendShapeGroups")
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(|group| {
                    let preset = group.get("presetName").and_then(Value::as_str).filter(|s| !s.is_empty());
                    let name = group.get("name").and_then(Value::as_str).filter(|s| !s.is_empty());
                    preset.or(name).map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gltf_bytes(json_chunk: &str) -> Vec<u8> {
        let json_bytes = json_chunk.as_bytes();
        let mut out = Vec::new();
        out.extend_from_slice(&GLTF_MAGIC.to_le_bytes());
        out.extend_from_slice(&2u32.to_le_bytes()); // version
        out.extend_from_slice(&((HEADER_BYTES + CHUNK_HEADER_BYTES + json_bytes.len()) as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        out.extend_from_slice(json_bytes);
        out
    }

    #[test]
    fn reads_the_json_chunk_of_a_well_formed_gltf_binary() {
        let scene = read_gltf_json_chunk(&gltf_bytes(r#"{"asset":{"version":"2.0"}}"#)).unwrap();
        assert_eq!(scene["asset"]["version"], json!("2.0"));
    }

    #[test]
    fn rejects_a_file_without_the_gltf_magic() {
        let error = read_gltf_json_chunk(b"not a real vrm file at all").unwrap_err();
        assert!(error.contains("glTF"), "mensagem devia nomear o formato esperado: {error}");
    }

    #[test]
    fn rejects_a_chunk_that_is_not_json() {
        let mut bytes = gltf_bytes("{}");
        // Troca o tipo do chunk (offset 16..20) de JSON (0x4E4F534A) para BIN (0x004E4942).
        bytes[16..20].copy_from_slice(&0x004e_4942u32.to_le_bytes());
        let error = read_gltf_json_chunk(&bytes).unwrap_err();
        assert!(error.contains("JSON"), "mensagem devia nomear o tipo de chunk esperado: {error}");
    }

    /// NEGATIVO — um `chunkLength` que mentiria sobre o tamanho do arquivo (`.vrm` truncado) falha
    /// alto, nunca faz `panic!` num slice fora dos limites.
    #[test]
    fn rejects_a_truncated_chunk_instead_of_panicking() {
        let mut bytes = gltf_bytes("{}");
        let inflated = 10_000_000u32.to_le_bytes();
        bytes[HEADER_BYTES..HEADER_BYTES + 4].copy_from_slice(&inflated);
        let error = read_gltf_json_chunk(&bytes).unwrap_err();
        assert!(error.contains("excede"), "mensagem devia nomear o estouro de tamanho: {error}");
    }

    #[test]
    fn vrm_expression_names_reads_vrm1_preset_and_custom() {
        let scene = json!({
            "extensions": { "VRMC_vrm": { "expressions": { "preset": { "happy": {} }, "custom": { "cool": {} } } } }
        });
        let mut names = vrm_expression_names(&scene);
        names.sort();
        assert_eq!(names, vec!["cool".to_string(), "happy".to_string()]);
    }

    /// NEGATIVO — VRM 0.x só é lido quando VRM 1.0 não declara nada (mesma ordem do TS).
    #[test]
    fn vrm_expression_names_falls_back_to_vrm0_blend_shape_groups() {
        let scene = json!({
            "extensions": { "VRM": { "blendShapeMaster": { "blendShapeGroups": [{ "presetName": "Joy" }, { "name": "Wink" }] } } }
        });
        assert_eq!(vrm_expression_names(&scene), vec!["Joy".to_string(), "Wink".to_string()]);
    }

    /// NEGATIVO — `presetName: ""` é falsy no TS (`group.presetName || group.name`), então cai
    /// para `name`, nunca vira uma expressão de nome vazio.
    #[test]
    fn vrm_expression_names_falls_back_to_name_when_preset_name_is_blank() {
        let scene = json!({
            "extensions": { "VRM": { "blendShapeMaster": { "blendShapeGroups": [{ "presetName": "", "name": "Custom" }] } } }
        });
        assert_eq!(vrm_expression_names(&scene), vec!["Custom".to_string()]);
    }

    #[test]
    fn vrm_expression_names_is_empty_when_neither_extension_is_present() {
        assert!(vrm_expression_names(&json!({})).is_empty());
    }
}
