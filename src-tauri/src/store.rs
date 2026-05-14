use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};
use crate::models::{
    Account, AccountInput, AttachmentMeta, Folder, MessageBody, MessageHeader, SocketSecurity,
};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> AppResult<()> {
        self.conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                email TEXT NOT NULL,
                imap_host TEXT NOT NULL,
                imap_port INTEGER NOT NULL,
                imap_security TEXT NOT NULL,
                smtp_host TEXT NOT NULL,
                smtp_port INTEGER NOT NULL,
                smtp_security TEXT NOT NULL,
                username TEXT NOT NULL,
                credential_key TEXT NOT NULL,
                is_default_sender INTEGER NOT NULL DEFAULT 0,
                sync_enabled INTEGER NOT NULL DEFAULT 1,
                last_error TEXT
            );

            CREATE TABLE IF NOT EXISTS folders (
                account_id TEXT NOT NULL,
                path TEXT NOT NULL,
                display_name TEXT NOT NULL,
                role TEXT,
                uid_validity INTEGER,
                last_synced_uid INTEGER,
                PRIMARY KEY (account_id, path),
                FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS message_headers (
                account_id TEXT NOT NULL,
                folder_path TEXT NOT NULL,
                uid INTEGER NOT NULL,
                message_id TEXT,
                subject TEXT NOT NULL,
                sender TEXT NOT NULL,
                recipients TEXT NOT NULL,
                cc TEXT,
                date TEXT NOT NULL,
                flags TEXT NOT NULL,
                has_attachments INTEGER NOT NULL DEFAULT 0,
                snippet TEXT,
                PRIMARY KEY (account_id, folder_path, uid),
                FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS idx_headers_unified
                ON message_headers(folder_path, date DESC);

            CREATE TABLE IF NOT EXISTS message_bodies (
                account_id TEXT NOT NULL,
                folder_path TEXT NOT NULL,
                uid INTEGER NOT NULL,
                text_body TEXT,
                sanitized_html_body TEXT,
                fetched_at TEXT NOT NULL,
                PRIMARY KEY (account_id, folder_path, uid),
                FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS attachment_meta (
                account_id TEXT NOT NULL,
                folder_path TEXT NOT NULL,
                uid INTEGER NOT NULL,
                filename TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                size INTEGER,
                part_id TEXT NOT NULL,
                PRIMARY KEY (account_id, folder_path, uid, part_id),
                FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
            );
            ",
        )?;
        Ok(())
    }

    pub fn upsert_account(&self, account: &Account) -> AppResult<()> {
        if account.is_default_sender {
            self.conn
                .execute("UPDATE accounts SET is_default_sender = 0", [])?;
        }

        self.conn.execute(
            "
            INSERT INTO accounts (
                id, display_name, email, imap_host, imap_port, imap_security,
                smtp_host, smtp_port, smtp_security, username, credential_key,
                is_default_sender, sync_enabled, last_error
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ON CONFLICT(id) DO UPDATE SET
                display_name = excluded.display_name,
                email = excluded.email,
                imap_host = excluded.imap_host,
                imap_port = excluded.imap_port,
                imap_security = excluded.imap_security,
                smtp_host = excluded.smtp_host,
                smtp_port = excluded.smtp_port,
                smtp_security = excluded.smtp_security,
                username = excluded.username,
                credential_key = excluded.credential_key,
                is_default_sender = excluded.is_default_sender,
                sync_enabled = excluded.sync_enabled,
                last_error = excluded.last_error
            ",
            params![
                account.id,
                account.display_name,
                account.email,
                account.imap_host,
                account.imap_port,
                account.imap_security.as_str(),
                account.smtp_host,
                account.smtp_port,
                account.smtp_security.as_str(),
                account.username,
                account.credential_key,
                account.is_default_sender as i64,
                account.sync_enabled as i64,
                account.last_error,
            ],
        )?;

        if self.default_account()?.is_none() {
            self.conn.execute(
                "UPDATE accounts SET is_default_sender = 1 WHERE id = ?1",
                params![account.id],
            )?;
        }

        Ok(())
    }

    pub fn update_account(&self, id: &str, input: &AccountInput) -> AppResult<Account> {
        let mut account = self
            .account(id)?
            .ok_or_else(|| AppError::Validation("account not found".to_string()))?;
        account.display_name = input.display_name.trim().to_string();
        account.email = input.email.trim().to_string();
        account.imap_host = input.imap_host.trim().to_string();
        account.imap_port = input.imap_port;
        account.imap_security = input.imap_security;
        account.smtp_host = input.smtp_host.trim().to_string();
        account.smtp_port = input.smtp_port;
        account.smtp_security = input.smtp_security;
        account.username = input.username.trim().to_string();
        account.is_default_sender = input.is_default_sender;
        account.sync_enabled = input.sync_enabled;
        self.upsert_account(&account)?;
        self.account(id)?
            .ok_or_else(|| AppError::Validation("account not found after update".to_string()))
    }

    pub fn remove_account(&self, id: &str) -> AppResult<Option<Account>> {
        let account = self.account(id)?;
        self.conn
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
        Ok(account)
    }

    pub fn accounts(&self) -> AppResult<Vec<Account>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id, display_name, email, imap_host, imap_port, imap_security,
                   smtp_host, smtp_port, smtp_security, username, credential_key,
                   is_default_sender, sync_enabled, last_error
            FROM accounts
            ORDER BY is_default_sender DESC, display_name ASC
            ",
        )?;
        let rows = stmt.query_map([], account_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn account(&self, id: &str) -> AppResult<Option<Account>> {
        self.conn
            .query_row(
                "
                SELECT id, display_name, email, imap_host, imap_port, imap_security,
                       smtp_host, smtp_port, smtp_security, username, credential_key,
                       is_default_sender, sync_enabled, last_error
                FROM accounts WHERE id = ?1
                ",
                params![id],
                account_from_row,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn default_account(&self) -> AppResult<Option<Account>> {
        self.conn
            .query_row(
                "
                SELECT id, display_name, email, imap_host, imap_port, imap_security,
                       smtp_host, smtp_port, smtp_security, username, credential_key,
                       is_default_sender, sync_enabled, last_error
                FROM accounts WHERE is_default_sender = 1 LIMIT 1
                ",
                [],
                account_from_row,
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn set_account_error(&self, account_id: &str, error: Option<&str>) -> AppResult<()> {
        self.conn.execute(
            "UPDATE accounts SET last_error = ?2 WHERE id = ?1",
            params![account_id, error],
        )?;
        Ok(())
    }

    pub fn upsert_folder(&self, folder: &Folder) -> AppResult<()> {
        self.conn.execute(
            "
            INSERT INTO folders (account_id, path, display_name, role, uid_validity, last_synced_uid)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(account_id, path) DO UPDATE SET
                display_name = excluded.display_name,
                role = excluded.role,
                uid_validity = excluded.uid_validity,
                last_synced_uid = excluded.last_synced_uid
            ",
            params![
                folder.account_id,
                folder.path,
                folder.display_name,
                folder.role,
                folder.uid_validity,
                folder.last_synced_uid,
            ],
        )?;
        Ok(())
    }

    pub fn folders(&self, account_id: &str) -> AppResult<Vec<Folder>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT account_id, path, display_name, role, uid_validity, last_synced_uid
            FROM folders
            WHERE account_id = ?1
            ORDER BY CASE role WHEN 'inbox' THEN 0 WHEN 'sent' THEN 1 ELSE 2 END, display_name
            ",
        )?;
        let rows = stmt.query_map(params![account_id], folder_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn upsert_message_header(&self, message: &MessageHeader) -> AppResult<()> {
        let flags = serde_json::to_string(&message.flags)
            .map_err(|err| AppError::Other(err.to_string()))?;
        self.conn.execute(
            "
            INSERT INTO message_headers (
                account_id, folder_path, uid, message_id, subject, sender, recipients,
                cc, date, flags, has_attachments, snippet
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(account_id, folder_path, uid) DO UPDATE SET
                message_id = excluded.message_id,
                subject = excluded.subject,
                sender = excluded.sender,
                recipients = excluded.recipients,
                cc = excluded.cc,
                date = excluded.date,
                flags = excluded.flags,
                has_attachments = excluded.has_attachments,
                snippet = excluded.snippet
            ",
            params![
                message.account_id,
                message.folder_path,
                message.uid,
                message.message_id,
                message.subject,
                message.from,
                message.to,
                message.cc,
                message.date.to_rfc3339(),
                flags,
                message.has_attachments as i64,
                message.snippet,
            ],
        )?;
        Ok(())
    }

    pub fn unified_inbox(&self, account_id: Option<&str>) -> AppResult<Vec<MessageHeader>> {
        self.messages(Some("INBOX"), account_id)
    }

    pub fn messages(&self, folder_path: Option<&str>, account_id: Option<&str>) -> AppResult<Vec<MessageHeader>> {
        let mut sql = String::from(
            "
            SELECT h.account_id, a.email, a.display_name, h.folder_path, h.uid,
                   h.message_id, h.subject, h.sender, h.recipients, h.cc, h.date,
                   h.flags, h.has_attachments, h.snippet
            FROM message_headers h
            JOIN accounts a ON a.id = h.account_id
            WHERE 1 = 1
            ",
        );
        if folder_path.is_some() {
            sql.push_str(" AND h.folder_path = ?1");
        }
        if account_id.is_some() {
            sql.push_str(if folder_path.is_some() {
                " AND h.account_id = ?2"
            } else {
                " AND h.account_id = ?1"
            });
        }
        sql.push_str(" ORDER BY h.date DESC LIMIT 500");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = match (folder_path, account_id) {
            (Some(folder), Some(account)) => {
                stmt.query_map(params![folder, account], message_header_from_row)?
            }
            (Some(folder), None) => stmt.query_map(params![folder], message_header_from_row)?,
            (None, Some(account)) => stmt.query_map(params![account], message_header_from_row)?,
            (None, None) => stmt.query_map([], message_header_from_row)?,
        };

        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn cached_body(&self, account_id: &str, folder_path: &str, uid: i64) -> AppResult<Option<MessageBody>> {
        let body = self
            .conn
            .query_row(
                "
                SELECT account_id, folder_path, uid, text_body, sanitized_html_body, fetched_at
                FROM message_bodies
                WHERE account_id = ?1 AND folder_path = ?2 AND uid = ?3
                ",
                params![account_id, folder_path, uid],
                |row| {
                    let fetched: String = row.get(5)?;
                    Ok(MessageBody {
                        account_id: row.get(0)?,
                        folder_path: row.get(1)?,
                        uid: row.get(2)?,
                        text_body: row.get(3)?,
                        sanitized_html_body: row.get(4)?,
                        fetched_at: parse_date(&fetched),
                        attachments: Vec::new(),
                    })
                },
            )
            .optional()?;

        match body {
            Some(mut body) => {
                body.attachments = self.attachments(account_id, folder_path, uid)?;
                Ok(Some(body))
            }
            None => Ok(None),
        }
    }

    pub fn upsert_body(&self, body: &MessageBody) -> AppResult<()> {
        self.conn.execute(
            "
            INSERT INTO message_bodies (
                account_id, folder_path, uid, text_body, sanitized_html_body, fetched_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(account_id, folder_path, uid) DO UPDATE SET
                text_body = excluded.text_body,
                sanitized_html_body = excluded.sanitized_html_body,
                fetched_at = excluded.fetched_at
            ",
            params![
                body.account_id,
                body.folder_path,
                body.uid,
                body.text_body,
                body.sanitized_html_body,
                body.fetched_at.to_rfc3339(),
            ],
        )?;

        self.conn.execute(
            "DELETE FROM attachment_meta WHERE account_id = ?1 AND folder_path = ?2 AND uid = ?3",
            params![body.account_id, body.folder_path, body.uid],
        )?;
        for attachment in &body.attachments {
            self.upsert_attachment(attachment)?;
        }
        Ok(())
    }

    fn attachments(&self, account_id: &str, folder_path: &str, uid: i64) -> AppResult<Vec<AttachmentMeta>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT account_id, folder_path, uid, filename, mime_type, size, part_id
            FROM attachment_meta
            WHERE account_id = ?1 AND folder_path = ?2 AND uid = ?3
            ORDER BY filename
            ",
        )?;
        let rows = stmt.query_map(params![account_id, folder_path, uid], attachment_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    fn upsert_attachment(&self, attachment: &AttachmentMeta) -> AppResult<()> {
        self.conn.execute(
            "
            INSERT INTO attachment_meta (account_id, folder_path, uid, filename, mime_type, size, part_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(account_id, folder_path, uid, part_id) DO UPDATE SET
                filename = excluded.filename,
                mime_type = excluded.mime_type,
                size = excluded.size
            ",
            params![
                attachment.account_id,
                attachment.folder_path,
                attachment.uid,
                attachment.filename,
                attachment.mime_type,
                attachment.size,
                attachment.part_id,
            ],
        )?;
        Ok(())
    }
}

fn account_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: row.get(0)?,
        display_name: row.get(1)?,
        email: row.get(2)?,
        imap_host: row.get(3)?,
        imap_port: row.get(4)?,
        imap_security: SocketSecurity::from_db(row.get::<_, String>(5)?.as_str()),
        smtp_host: row.get(6)?,
        smtp_port: row.get(7)?,
        smtp_security: SocketSecurity::from_db(row.get::<_, String>(8)?.as_str()),
        username: row.get(9)?,
        credential_key: row.get(10)?,
        is_default_sender: row.get::<_, i64>(11)? == 1,
        sync_enabled: row.get::<_, i64>(12)? == 1,
        last_error: row.get(13)?,
    })
}

fn folder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        account_id: row.get(0)?,
        path: row.get(1)?,
        display_name: row.get(2)?,
        role: row.get(3)?,
        uid_validity: row.get(4)?,
        last_synced_uid: row.get(5)?,
    })
}

fn message_header_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MessageHeader> {
    let flags_json: String = row.get(11)?;
    let flags = serde_json::from_str(&flags_json).unwrap_or_default();
    let date: String = row.get(10)?;
    Ok(MessageHeader {
        account_id: row.get(0)?,
        account_email: row.get(1)?,
        account_display_name: row.get(2)?,
        folder_path: row.get(3)?,
        uid: row.get(4)?,
        message_id: row.get(5)?,
        subject: row.get(6)?,
        from: row.get(7)?,
        to: row.get(8)?,
        cc: row.get(9)?,
        date: parse_date(&date),
        flags,
        has_attachments: row.get::<_, i64>(12)? == 1,
        snippet: row.get(13)?,
    })
}

fn attachment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttachmentMeta> {
    Ok(AttachmentMeta {
        account_id: row.get(0)?,
        folder_path: row.get(1)?,
        uid: row.get(2)?,
        filename: row.get(3)?,
        mime_type: row.get(4)?,
        size: row.get(5)?,
        part_id: row.get(6)?,
    })
}

fn parse_date(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(id: &str, default: bool) -> Account {
        Account {
            id: id.to_string(),
            display_name: format!("Account {id}"),
            email: format!("{id}@example.com"),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            imap_security: SocketSecurity::Tls,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
            smtp_security: SocketSecurity::Tls,
            username: format!("{id}@example.com"),
            credential_key: format!("credential-{id}"),
            is_default_sender: default,
            sync_enabled: true,
            last_error: None,
        }
    }

    #[test]
    fn default_sender_is_unique() {
        let store = Store::in_memory().unwrap();
        store.upsert_account(&account("a", true)).unwrap();
        store.upsert_account(&account("b", true)).unwrap();

        let accounts = store.accounts().unwrap();
        assert_eq!(
            accounts.iter().filter(|account| account.is_default_sender).count(),
            1
        );
        assert_eq!(store.default_account().unwrap().unwrap().id, "b");
    }

    #[test]
    fn unified_inbox_sorts_across_accounts() {
        let store = Store::in_memory().unwrap();
        store.upsert_account(&account("a", true)).unwrap();
        store.upsert_account(&account("b", false)).unwrap();

        for (account_id, uid, date) in [
            ("a", 1, "2026-05-13T08:00:00Z"),
            ("b", 2, "2026-05-14T08:00:00Z"),
        ] {
            store
                .upsert_message_header(&MessageHeader {
                    account_id: account_id.to_string(),
                    account_email: format!("{account_id}@example.com"),
                    account_display_name: account_id.to_string(),
                    folder_path: "INBOX".to_string(),
                    uid,
                    message_id: None,
                    subject: format!("Subject {uid}"),
                    from: "sender@example.com".to_string(),
                    to: "me@example.com".to_string(),
                    cc: None,
                    date: parse_date(date),
                    flags: vec![],
                    has_attachments: false,
                    snippet: None,
                })
                .unwrap();
        }

        let inbox = store.unified_inbox(None).unwrap();
        assert_eq!(inbox[0].account_id, "b");
        assert_eq!(store.unified_inbox(Some("a")).unwrap().len(), 1);
    }
}
