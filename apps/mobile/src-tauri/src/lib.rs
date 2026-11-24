mod avatar;
mod chat;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(chat::ChatClient::default())
        .invoke_handler(tauri::generate_handler![avatar::avatar_model, chat::chat])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar a Mika mobile");
}
