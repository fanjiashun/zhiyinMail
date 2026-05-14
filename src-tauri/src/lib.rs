mod commands;
mod credentials;
mod error;
mod mail;
mod models;
mod state;
mod store;

use commands::*;
use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new().expect("failed to initialize application state"))
        .invoke_handler(tauri::generate_handler![
            add_account,
            update_account,
            remove_account,
            test_account_connection,
            list_accounts,
            list_folders,
            sync_account_inbox,
            sync_all_inboxes,
            list_unified_inbox,
            list_account_messages,
            get_message_body,
            download_attachment,
            send_message,
            mark_read,
            mark_unread,
            delete_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running Unified Mail");
}
