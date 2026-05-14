use tauri::State;
use uuid::Uuid;

use crate::credentials::CredentialStore;
use crate::error::{AppError, AppResult};
use crate::mail::MailClient;
use crate::models::{
    Account, AccountInput, ConnectionReport, Folder, MessageBody, MessageHeader, SendMessageInput,
    SyncReport,
};
use crate::state::AppState;

#[tauri::command]
pub fn add_account(state: State<'_, AppState>, input: AccountInput) -> AppResult<Account> {
    validate_account_input(&input, true)?;
    let id = Uuid::new_v4().to_string();
    let credential_key = format!("account:{id}");
    let password = input
        .password
        .as_deref()
        .ok_or_else(|| AppError::Validation("password is required".to_string()))?;
    state.credentials.set_password(&credential_key, password)?;

    let account = Account {
        id: id.clone(),
        display_name: input.display_name.trim().to_string(),
        email: input.email.trim().to_string(),
        imap_host: input.imap_host.trim().to_string(),
        imap_port: input.imap_port,
        imap_security: input.imap_security,
        smtp_host: input.smtp_host.trim().to_string(),
        smtp_port: input.smtp_port,
        smtp_security: input.smtp_security,
        username: input.username.trim().to_string(),
        credential_key,
        is_default_sender: input.is_default_sender,
        sync_enabled: input.sync_enabled,
        last_error: None,
    };

    with_store(&state, |store| store.upsert_account(&account))?;
    Ok(with_store(&state, |store| store.account(&id))?
        .ok_or_else(|| AppError::Other("account disappeared after save".to_string()))?)
}

#[tauri::command]
pub fn update_account(state: State<'_, AppState>, id: String, input: AccountInput) -> AppResult<Account> {
    validate_account_input(&input, false)?;
    let account = with_store(&state, |store| store.update_account(&id, &input))?;
    if let Some(password) = input.password.as_deref().filter(|value| !value.is_empty()) {
        state
            .credentials
            .set_password(&account.credential_key, password)?;
    }
    Ok(account)
}

#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, id: String) -> AppResult<()> {
    if let Some(account) = with_store(&state, |store| store.remove_account(&id))? {
        let _ = state.credentials.delete_password(&account.credential_key);
    }
    Ok(())
}

#[tauri::command]
pub fn test_account_connection(
    state: State<'_, AppState>,
    input: AccountInput,
) -> AppResult<ConnectionReport> {
    validate_account_input(&input, true)?;
    let account = Account {
        id: "connection-test".to_string(),
        display_name: input.display_name,
        email: input.email,
        imap_host: input.imap_host,
        imap_port: input.imap_port,
        imap_security: input.imap_security,
        smtp_host: input.smtp_host,
        smtp_port: input.smtp_port,
        smtp_security: input.smtp_security,
        username: input.username,
        credential_key: "connection-test".to_string(),
        is_default_sender: input.is_default_sender,
        sync_enabled: input.sync_enabled,
        last_error: None,
    };
    let password = input
        .password
        .as_deref()
        .ok_or_else(|| AppError::Validation("password is required".to_string()))?;
    let _ = state;
    MailClient::test_connection(&account, password)
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> AppResult<Vec<Account>> {
    with_store(&state, |store| store.accounts())
}

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>, account_id: String) -> AppResult<Vec<Folder>> {
    let cached = with_store(&state, |store| store.folders(&account_id))?;
    if !cached.is_empty() {
        return Ok(cached);
    }

    let account = require_account(&state, &account_id)?;
    let password = state.credentials.get_password(&account.credential_key)?;
    let folders = MailClient::list_folders(&account, &password)?;
    with_store(&state, |store| {
        for folder in &folders {
            store.upsert_folder(folder)?;
        }
        Ok(())
    })?;
    Ok(folders)
}

#[tauri::command]
pub fn sync_account_inbox(state: State<'_, AppState>, account_id: String) -> AppResult<SyncReport> {
    let account = require_account(&state, &account_id)?;
    sync_one_account(&state, &account)
}

#[tauri::command]
pub fn sync_all_inboxes(state: State<'_, AppState>) -> AppResult<Vec<SyncReport>> {
    let accounts = with_store(&state, |store| store.accounts())?;
    let mut reports = Vec::new();
    for account in accounts.into_iter().filter(|account| account.sync_enabled) {
        reports.push(sync_one_account(&state, &account)?);
    }
    Ok(reports)
}

#[tauri::command]
pub fn list_unified_inbox(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> AppResult<Vec<MessageHeader>> {
    with_store(&state, |store| store.unified_inbox(account_id.as_deref()))
}

#[tauri::command]
pub fn list_account_messages(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: Option<String>,
) -> AppResult<Vec<MessageHeader>> {
    with_store(&state, |store| {
        store.messages(folder_path.as_deref(), Some(account_id.as_str()))
    })
}

#[tauri::command]
pub fn get_message_body(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: String,
    uid: i64,
) -> AppResult<MessageBody> {
    if let Some(cached) = with_store(&state, |store| store.cached_body(&account_id, &folder_path, uid))? {
        return Ok(cached);
    }

    let account = require_account(&state, &account_id)?;
    let password = state.credentials.get_password(&account.credential_key)?;
    let body = MailClient::get_body(&account, &password, &folder_path, uid)?;
    with_store(&state, |store| store.upsert_body(&body))?;
    Ok(body)
}

#[tauri::command]
pub fn download_attachment(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: String,
    uid: i64,
    part_id: String,
    output_path: String,
) -> AppResult<()> {
    let account = require_account(&state, &account_id)?;
    let password = state.credentials.get_password(&account.credential_key)?;
    MailClient::download_attachment(&account, &password, &folder_path, uid, &part_id, &output_path)
}

#[tauri::command]
pub fn send_message(state: State<'_, AppState>, input: SendMessageInput) -> AppResult<()> {
    if input.subject.trim().is_empty() {
        return Err(AppError::Validation("subject is required".to_string()));
    }
    let account = require_account(&state, &input.account_id)?;
    let password = state.credentials.get_password(&account.credential_key)?;
    MailClient::send_message(&account, &password, &input)
}

#[tauri::command]
pub fn mark_read(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: String,
    uid: i64,
) -> AppResult<()> {
    set_seen(state, account_id, folder_path, uid, true)
}

#[tauri::command]
pub fn mark_unread(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: String,
    uid: i64,
) -> AppResult<()> {
    set_seen(state, account_id, folder_path, uid, false)
}

#[tauri::command]
pub fn delete_message(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: String,
    uid: i64,
) -> AppResult<()> {
    let account = require_account(&state, &account_id)?;
    let password = state.credentials.get_password(&account.credential_key)?;
    MailClient::delete_message(&account, &password, &folder_path, uid)
}

fn sync_one_account(state: &State<'_, AppState>, account: &Account) -> AppResult<SyncReport> {
    let password = state.credentials.get_password(&account.credential_key)?;
    match MailClient::sync_inbox(account, &password, 100) {
        Ok(headers) => {
            with_store(state, |store| {
                store.set_account_error(&account.id, None)?;
                store.upsert_folder(&Folder {
                    account_id: account.id.clone(),
                    path: "INBOX".to_string(),
                    display_name: "Inbox".to_string(),
                    role: Some("inbox".to_string()),
                    uid_validity: None,
                    last_synced_uid: headers.iter().map(|message| message.uid).max(),
                })?;
                for header in &headers {
                    store.upsert_message_header(header)?;
                }
                Ok(())
            })?;
            Ok(SyncReport {
                account_id: account.id.clone(),
                synced: headers.len(),
                error: None,
            })
        }
        Err(err) => {
            let message = err.to_string();
            with_store(state, |store| store.set_account_error(&account.id, Some(&message)))?;
            Ok(SyncReport {
                account_id: account.id.clone(),
                synced: 0,
                error: Some(message),
            })
        }
    }
}

fn set_seen(
    state: State<'_, AppState>,
    account_id: String,
    folder_path: String,
    uid: i64,
    seen: bool,
) -> AppResult<()> {
    let account = require_account(&state, &account_id)?;
    let password = state.credentials.get_password(&account.credential_key)?;
    MailClient::mark_seen(&account, &password, &folder_path, uid, seen)
}

fn require_account(state: &State<'_, AppState>, account_id: &str) -> AppResult<Account> {
    with_store(state, |store| store.account(account_id))?
        .ok_or_else(|| AppError::Validation("account not found".to_string()))
}

fn with_store<T>(
    state: &State<'_, AppState>,
    action: impl FnOnce(&crate::store::Store) -> AppResult<T>,
) -> AppResult<T> {
    let guard = state
        .store
        .lock()
        .map_err(|_| AppError::Other("store lock poisoned".to_string()))?;
    action(&guard)
}

fn validate_account_input(input: &AccountInput, require_password: bool) -> AppResult<()> {
    if input.display_name.trim().is_empty() {
        return Err(AppError::Validation("display name is required".to_string()));
    }
    if input.email.trim().is_empty() || !input.email.contains('@') {
        return Err(AppError::Validation("valid email is required".to_string()));
    }
    if input.username.trim().is_empty() {
        return Err(AppError::Validation("username is required".to_string()));
    }
    if input.imap_host.trim().is_empty() || input.smtp_host.trim().is_empty() {
        return Err(AppError::Validation("IMAP and SMTP hosts are required".to_string()));
    }
    if input.imap_port == 0 || input.smtp_port == 0 {
        return Err(AppError::Validation("IMAP and SMTP ports are required".to_string()));
    }
    if require_password && input.password.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::Validation("password is required".to_string()));
    }
    Ok(())
}
