//! Porte puro de `services/model-inventory.ts` + `services/vrm-inventory.ts`
//! (`apps/desktop/src/features/character/services/`). Nenhuma dependência externa: `.model3.json`
//! é JSON puro (`serde_json` já é dependência do crate) e `.vrm` é glTF BINÁRIO cujo primeiro chunk
//! também é só JSON — 12 bytes de cabeçalho fixo + 8 bytes de comprimento/tipo do chunk, exatamente
//! os bytes que `readGltfJsonChunk` (TS) já lia à mão, sem nenhuma lib de glTF do lado TypeScript
//! também. Ver o cabeçalho de `inspect_model.rs` para a medição completa do que isso resolve.

use serde::Serialize;
use serde_json::{Map, Value};

use super::vrm_gltf::{read_gltf_json_chunk, vrm_expression_names};

/// `EXPRESSIONS` (`apps/desktop/src/features/character/tools/avatar.ts`) — o que `action='expression'`
/// aceita e o que `action='inspect_model'` devolve como `supportedExpressions`. Mesma ordem do TS.
pub const EXPRESSIONS: &[&str] = &[
    "happy",
    "sad",
    "surprised",
    "angry",
    "neutral",
    "thinking",
    "excited",
    "confused",
    "embarrassed",
    "serious",
    "sleep",
    "smug",
    "wink",
    "love",
    "evil",
    "panic",
    "bored",
    "laugh",
    "disgusted",
    "scared",
    "determined",
    "tired",
    "shy",
    "sitting",
    "reading",
    "curious",
    "salute",
    "whisper",
    "celebrate",
    "relaxed",
    "pout",
    "facepalm",
    "sigh",
    "beckon",
    "wave",
    "bow",
    "stretch",
    "peace",
    "22",
    "heart",
    "victory",
    "nod",
    "shrug",
    "point",
    "six-seven",
    "floss",
    "orange-justice",
    "yawn",
    "drink",
    "throw-tomato",
    "senbonzakura",
    "clap",
    "thumbs-up",
    "headshake",
    "bounce",
    "slump",
    "focus",
    "groove",
    "doze",
    "fist-pump",
    "gasp",
    "rage",
    "tada",
];

/// `VRM_BODY_ACTIONS` (`packages/sdk/src/contracts/types/avatar-types.ts`) — as "motions" de corpo
/// que um VRM sempre tem, porque o rig mora no renderer (`vrm-body-rig.ts`), não no arquivo.
pub const VRM_BODY_ACTIONS: &[&str] = &[
    "wave",
    "shrug",
    "cheer",
    "shy",
    "thinking",
    "laugh",
    "bow",
    "stretch",
    "presenting",
    "peace",
    "22",
    "heart",
    "victory",
    "pout",
    "facepalm",
    "smug",
    "surprised",
    "sigh",
    "beckon",
    "curious",
    "salute",
    "whisper",
    "celebrate",
    "relaxed",
    "reading",
    "sitting",
    "point",
    "six-seven",
    "floss",
    "orange-justice",
    "yawn",
    "drink",
    "throw-tomato",
    "senbonzakura",
    "clap",
    "thumbs-up",
    "headshake",
    "bounce",
    "slump",
    "focus",
    "groove",
    "doze",
    "fist-pump",
    "gasp",
    "rage",
    "tada",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExpressionInfo {
    pub name: String,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MotionInfo {
    pub group: String,
    pub name: String,
    pub file: String,
}

#[derive(Debug)]
pub struct ModelInventory {
    pub kind: &'static str,
    pub expressions: Vec<ExpressionInfo>,
    pub motions: Vec<MotionInfo>,
}

/// Porte de `live2dInventory`: lê `FileReferences.Expressions`/`.Motions`, com fallback para os
/// campos de topo `Expressions`/`Motions` (arquivo antigo) — mesma ordem de preferência do TS.
pub fn live2d_inventory(model: &Value) -> ModelInventory {
    let refs = model.get("FileReferences");
    let expressions = refs
        .and_then(|r| r.get("Expressions"))
        .or_else(|| model.get("Expressions"))
        .and_then(Value::as_array)
        .map(|items| items.iter().map(normalize_expression).collect())
        .unwrap_or_default();
    let motions = refs
        .and_then(|r| r.get("Motions"))
        .or_else(|| model.get("Motions"))
        .and_then(Value::as_object)
        .map(normalize_motions)
        .unwrap_or_default();

    ModelInventory { kind: "live2d", expressions, motions }
}

/// `.vrm`: expressões saem do arquivo (VRM 1.0 `VRMC_vrm.expressions`, senão VRM 0.x
/// `blendShapeMaster`); as "motions" são as ações de corpo do rig — o rig mora no renderer, então a
/// lista é o contrato compartilhado (`VRM_BODY_ACTIONS`), nunca lida do arquivo.
pub fn vrm_inventory(bytes: &[u8]) -> Result<ModelInventory, String> {
    let scene = read_gltf_json_chunk(bytes)?;
    let expressions =
        vrm_expression_names(&scene).into_iter().map(|name| ExpressionInfo { name, file: String::new() }).collect();
    let motions = VRM_BODY_ACTIONS
        .iter()
        .map(|name| MotionInfo { group: "body".to_string(), name: (*name).to_string(), file: String::new() })
        .collect();

    Ok(ModelInventory { kind: "vrm", expressions, motions })
}

fn normalize_expression(expression: &Value) -> ExpressionInfo {
    let file = expression.get("File").and_then(Value::as_str).unwrap_or_default().to_string();
    let fallback = file.rsplit('/').next().map(|s| s.replace(".exp3.json", "")).filter(|s| !s.is_empty());
    let name = expression
        .get("Name")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or(fallback)
        .unwrap_or_else(|| "unknown".to_string());
    ExpressionInfo { name, file }
}

fn normalize_motions(motions: &Map<String, Value>) -> Vec<MotionInfo> {
    let mut out = Vec::new();
    for (group, items) in motions {
        let Some(items) = items.as_array() else { continue };
        for (index, motion) in items.iter().enumerate() {
            let file = motion.get("File").and_then(Value::as_str).unwrap_or_default().to_string();
            let fallback = file.rsplit('/').next().map(|s| s.replace(".motion3.json", "")).filter(|s| !s.is_empty());
            let name = fallback.unwrap_or_else(|| format!("{group}_{}", index + 1));
            out.push(MotionInfo { group: group.clone(), name: format!("{group}:{name}"), file });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn live2d_inventory_reads_expressions_and_motions_from_file_references() {
        let model = json!({
            "FileReferences": {
                "Expressions": [{ "Name": "F01", "File": "expressions/f01.exp3.json" }],
                "Motions": { "Idle": [{ "File": "motions/idle_0.motion3.json" }] },
            }
        });
        let inventory = live2d_inventory(&model);
        assert_eq!(inventory.kind, "live2d");
        assert_eq!(inventory.expressions, vec![ExpressionInfo { name: "F01".into(), file: "expressions/f01.exp3.json".into() }]);
        assert_eq!(
            inventory.motions,
            vec![MotionInfo { group: "Idle".into(), name: "Idle:idle_0".into(), file: "motions/idle_0.motion3.json".into() }]
        );
    }

    /// NEGATIVO — sem `Name`, o nome cai para o basename do arquivo sem a extensão `.exp3.json`.
    #[test]
    fn live2d_inventory_falls_back_to_the_file_basename_when_name_is_missing() {
        let model = json!({ "Expressions": [{ "File": "expressions/happy.exp3.json" }] });
        let inventory = live2d_inventory(&model);
        assert_eq!(inventory.expressions[0].name, "happy");
    }

    #[test]
    fn live2d_inventory_is_empty_when_the_model_declares_neither() {
        let inventory = live2d_inventory(&json!({ "FileReferences": {} }));
        assert!(inventory.expressions.is_empty());
        assert!(inventory.motions.is_empty());
    }

    // O parsing do glTF binário em si (magic, chunk JSON, truncamento, VRM0 x VRM1) é coberto em
    // `vrm_gltf.rs` — aqui só a fiação de `vrm_inventory` por cima dele: `ExpressionInfo`/
    // `MotionInfo` montados certo, e `motions` sempre o rig de corpo inteiro (`VRM_BODY_ACTIONS`).
    fn gltf_bytes(json_chunk: &str) -> Vec<u8> {
        let json_bytes = json_chunk.as_bytes();
        let mut out = vec![0x67, 0x6c, 0x54, 0x46]; // magic "glTF" (little-endian)
        out.extend_from_slice(&2u32.to_le_bytes()); // version
        out.extend_from_slice(&((20 + json_bytes.len()) as u32).to_le_bytes()); // total length
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes()); // chunk length
        out.extend_from_slice(&[0x4a, 0x53, 0x4f, 0x4e]); // chunk type "JSON"
        out.extend_from_slice(json_bytes);
        out
    }

    #[test]
    fn vrm_inventory_wires_expressions_and_the_full_body_rig() {
        let scene = json!({ "extensions": { "VRMC_vrm": { "expressions": { "preset": { "happy": {} }, "custom": {} } } } });
        let inventory = vrm_inventory(&gltf_bytes(&scene.to_string())).unwrap();
        assert_eq!(inventory.kind, "vrm");
        assert_eq!(inventory.expressions, vec![ExpressionInfo { name: "happy".into(), file: String::new() }]);
        assert_eq!(inventory.motions.len(), VRM_BODY_ACTIONS.len(), "motions do VRM são sempre o rig de corpo inteiro");
        assert!(inventory.motions.iter().all(|m| m.group == "body" && !m.name.is_empty()));
    }

    #[test]
    fn vrm_inventory_propagates_a_malformed_file_as_a_named_error() {
        let error = vrm_inventory(b"not a real vrm file at all").unwrap_err();
        assert!(error.contains("glTF"), "mensagem devia nomear o formato esperado: {error}");
    }
}
