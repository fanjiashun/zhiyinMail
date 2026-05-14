use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SocketSecurity {
    Tls,
    StartTls,
    Plain,
}

impl SocketSecurity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::StartTls => "startTls",
            Self::Plain => "plain",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "startTls" => Self::StartTls,
            "plain" => Self::Plain,
            _ => Self::Tls,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub display_name: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SocketSecurity,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SocketSecurity,
    pub username: String,
    pub credential_key: String,
    pub is_default_sender: bool,
    pub sync_enabled: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub display_name: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_security: SocketSecurity,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_security: SocketSecurity,
    pub username: String,
    pub password: Option<String>,
    pub is_default_sender: bool,
    pub sync_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub account_id: String,
    pub path: String,
    pub display_name: String,
    pub role: Option<String>,
    pub uid_validity: Option<i64>,
    pub last_synced_uid: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageHeader {
    pub account_id: String,
    pub account_email: String,
    pub account_display_name: String,
    pub folder_path: String,
    pub uid: i64,
    pub message_id: Option<String>,
    pub subject: String,
    pub from: String,
    pub to: String,
    pub cc: Option<String>,
    pub date: DateTime<Utc>,
    pub flags: Vec<String>,
    pub has_attachments: bool,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBody {
    pub account_id: String,
    pub folder_path: String,
    pub uid: i64,
    pub text_body: Option<String>,
    pub sanitized_html_body: Option<String>,
    pub fetched_at: DateTime<Utc>,
    pub attachments: Vec<AttachmentMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub account_id: String,
    pub folder_path: String,
    pub uid: i64,
    pub filename: String,
    pub mime_type: String,
    pub size: Option<i64>,
    pub part_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageInput {
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub account_id: String,
    pub synced: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionReport {
    pub imap_ok: bool,
    pub smtp_ok: bool,
    pub message: String,
}
