// Served over IPC: the 16 MB VRM embedded as a frontend asset failed to fetch from the Android
// WebView ("Failed to fetch"), while small assets loaded fine.
const MIKA_VRM: &[u8] = include_bytes!("../../../desktop/src/renderer/public/vrm/mika/mika.vrm");

#[tauri::command]
pub fn avatar_model() -> tauri::ipc::Response {
    tauri::ipc::Response::new(MIKA_VRM.to_vec())
}
