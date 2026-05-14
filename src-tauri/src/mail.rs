use std::io::Write;
use std::net::TcpStream;

use chrono::{DateTime, Utc};
use html_escape::encode_safe;
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use mail_parser::{Addr, Address, MessageParser, MimeHeaders, PartType};
use native_tls::{TlsConnector, TlsStream};

use crate::error::{AppError, AppResult};
use crate::models::{
    Account, AttachmentMeta, ConnectionReport, Folder, MessageBody, MessageHeader, SendMessageInput,
    SocketSecurity,
};

type ImapSession = imap::Session<TlsStream<TcpStream>>;

pub struct MailClient;

impl MailClient {
    pub fn test_connection(account: &Account, password: &str) -> AppResult<ConnectionReport> {
        let imap_ok = Self::connect_imap(account, password).is_ok();
        let smtp_ok = Self::smtp_transport(account, password)
            .and_then(|transport| transport.test_connection().map_err(|err| AppError::Mail(err.to_string())))
            .unwrap_or(false);
        let message = match (imap_ok, smtp_ok) {
            (true, true) => "IMAP and SMTP connections succeeded.",
            (true, false) => "IMAP succeeded, but SMTP failed.",
            (false, true) => "SMTP succeeded, but IMAP failed.",
            (false, false) => "IMAP and SMTP connections failed.",
        };

        Ok(ConnectionReport {
            imap_ok,
            smtp_ok,
            message: message.to_string(),
        })
    }

    pub fn list_folders(account: &Account, password: &str) -> AppResult<Vec<Folder>> {
        let mut session = Self::connect_imap(account, password)?;
        let folders = session
            .list(None, Some("*"))
            .map_err(|err| AppError::Mail(err.to_string()))?
            .iter()
            .map(|folder| {
                let path = folder.name().to_string();
                Folder {
                    account_id: account.id.clone(),
                    display_name: display_folder_name(&path),
                    role: folder_role(&path),
                    path,
                    uid_validity: None,
                    last_synced_uid: None,
                }
            })
            .collect();
        let _ = session.logout();
        Ok(folders)
    }

    pub fn sync_inbox(account: &Account, password: &str, limit: usize) -> AppResult<Vec<MessageHeader>> {
        let mut session = Self::connect_imap(account, password)?;
        session
            .select("INBOX")
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let mailbox = session
            .status("INBOX", "(MESSAGES)")
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let total = mailbox.exists;
        if total == 0 {
            let _ = session.logout();
            return Ok(Vec::new());
        }

        let start = total.saturating_sub(limit as u32).max(1);
        let sequence = format!("{start}:{total}");
        let messages = session
            .fetch(sequence, "(UID FLAGS BODY.PEEK[HEADER.FIELDS (MESSAGE-ID FROM TO CC SUBJECT DATE)])")
            .map_err(|err| AppError::Mail(err.to_string()))?;

        let mut headers = Vec::new();
        for message in messages.iter() {
            let Some(uid) = message.uid else {
                continue;
            };
            let Some(body) = message.body() else {
                continue;
            };
            headers.push(parse_header(account, "INBOX", uid as i64, message.flags(), body));
        }

        let _ = session.logout();
        headers.sort_by(|left, right| right.date.cmp(&left.date));
        Ok(headers)
    }

    pub fn get_body(account: &Account, password: &str, folder_path: &str, uid: i64) -> AppResult<MessageBody> {
        let mut session = Self::connect_imap(account, password)?;
        session
            .select(folder_path)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let messages = session
            .uid_fetch(uid.to_string(), "BODY.PEEK[]")
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let message = messages
            .iter()
            .next()
            .ok_or_else(|| AppError::Mail("message not found".to_string()))?;
        let raw = message
            .body()
            .ok_or_else(|| AppError::Mail("message body missing".to_string()))?;
        let body = parse_body(account, folder_path, uid, raw)?;
        let _ = session.logout();
        Ok(body)
    }

    pub fn download_attachment(
        account: &Account,
        password: &str,
        folder_path: &str,
        uid: i64,
        part_id: &str,
        output_path: &str,
    ) -> AppResult<()> {
        let mut session = Self::connect_imap(account, password)?;
        session
            .select(folder_path)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let query = format!("BODY.PEEK[{part_id}]");
        let messages = session
            .uid_fetch(uid.to_string(), query)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let message = messages
            .iter()
            .next()
            .ok_or_else(|| AppError::Mail("attachment not found".to_string()))?;
        let body = message
            .body()
            .ok_or_else(|| AppError::Mail("attachment body missing".to_string()))?;
        let mut file = std::fs::File::create(output_path)?;
        file.write_all(body)?;
        let _ = session.logout();
        Ok(())
    }

    pub fn send_message(account: &Account, password: &str, input: &SendMessageInput) -> AppResult<()> {
        if input.to.is_empty() {
            return Err(AppError::Validation("at least one recipient is required".to_string()));
        }

        let mut builder = Message::builder()
            .from(parse_mailbox(&account.email)?)
            .subject(input.subject.trim());

        for recipient in &input.to {
            builder = builder.to(parse_mailbox(recipient)?);
        }
        for recipient in &input.cc {
            builder = builder.cc(parse_mailbox(recipient)?);
        }
        for recipient in &input.bcc {
            builder = builder.bcc(parse_mailbox(recipient)?);
        }

        let mut multipart = MultiPart::mixed().singlepart(SinglePart::plain(input.body.clone()));
        for path in &input.attachments {
            let bytes = std::fs::read(path)?;
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("attachment")
                .to_string();
            let attachment = lettre::message::Attachment::new(filename)
                .body(bytes, "application/octet-stream".parse().unwrap());
            multipart = multipart.singlepart(attachment);
        }

        let email = builder
            .multipart(multipart)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        Self::smtp_transport(account, password)?
            .send(&email)
            .map_err(|err| AppError::Mail(err.to_string()))?;

        Ok(())
    }

    pub fn mark_seen(account: &Account, password: &str, folder_path: &str, uid: i64, seen: bool) -> AppResult<()> {
        let mut session = Self::connect_imap(account, password)?;
        session
            .select(folder_path)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let command = if seen {
            "+FLAGS.SILENT (\\Seen)"
        } else {
            "-FLAGS.SILENT (\\Seen)"
        };
        session
            .uid_store(uid.to_string(), command)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let _ = session.logout();
        Ok(())
    }

    pub fn delete_message(account: &Account, password: &str, folder_path: &str, uid: i64) -> AppResult<()> {
        let mut session = Self::connect_imap(account, password)?;
        session
            .select(folder_path)
            .map_err(|err| AppError::Mail(err.to_string()))?;
        session
            .uid_store(uid.to_string(), "+FLAGS.SILENT (\\Deleted)")
            .map_err(|err| AppError::Mail(err.to_string()))?;
        session.expunge().map_err(|err| AppError::Mail(err.to_string()))?;
        let _ = session.logout();
        Ok(())
    }

    fn connect_imap(account: &Account, password: &str) -> AppResult<ImapSession> {
        if matches!(account.imap_security, SocketSecurity::Plain) {
            return Err(AppError::Validation(
                "plain IMAP is disabled in the first release".to_string(),
            ));
        }
        if matches!(account.imap_security, SocketSecurity::StartTls) {
            return Err(AppError::Validation(
                "STARTTLS IMAP is not implemented yet; use TLS port 993".to_string(),
            ));
        }

        let tls = TlsConnector::builder()
            .build()
            .map_err(|err| AppError::Mail(err.to_string()))?;
        let client = imap::connect(
            (&account.imap_host[..], account.imap_port),
            &account.imap_host,
            &tls,
        )
        .map_err(|err| AppError::Mail(err.to_string()))?;
        client
            .login(&account.username, password)
            .map_err(|(err, _)| AppError::Mail(err.to_string()))
    }

    fn smtp_transport(account: &Account, password: &str) -> AppResult<SmtpTransport> {
        if matches!(account.smtp_security, SocketSecurity::Plain) {
            return Err(AppError::Validation(
                "plain SMTP is disabled in the first release".to_string(),
            ));
        }

        let credentials = Credentials::new(account.username.clone(), password.to_string());
        let builder = match account.smtp_security {
            SocketSecurity::Tls => SmtpTransport::relay(&account.smtp_host),
            SocketSecurity::StartTls => SmtpTransport::starttls_relay(&account.smtp_host),
            SocketSecurity::Plain => unreachable!(),
        }
        .map_err(|err| AppError::Mail(err.to_string()))?;

        Ok(builder
            .port(account.smtp_port)
            .credentials(credentials)
            .build())
    }
}

fn parse_header(
    account: &Account,
    folder_path: &str,
    uid: i64,
    flags: &[imap::types::Flag<'_>],
    raw: &[u8],
) -> MessageHeader {
    let parsed = MessageParser::default().parse(raw);
    let subject = parsed
        .as_ref()
        .and_then(|message| message.subject())
        .unwrap_or("(No subject)")
        .to_string();
    let from = parsed
        .as_ref()
        .and_then(|message| message.from())
        .map(format_addresses)
        .unwrap_or_default();
    let to = parsed
        .as_ref()
        .and_then(|message| message.to())
        .map(format_addresses)
        .unwrap_or_default();
    let cc = parsed
        .as_ref()
        .and_then(|message| message.cc())
        .map(format_addresses);
    let message_id = parsed
        .as_ref()
        .and_then(|message| message.message_id())
        .map(|id| id.to_string());
    let date = parsed
        .as_ref()
        .and_then(|message| message.date())
        .and_then(|date| DateTime::from_timestamp(date.to_timestamp(), 0))
        .unwrap_or_else(Utc::now);
    let flag_values = flags.iter().map(|flag| flag.to_string()).collect::<Vec<_>>();

    MessageHeader {
        account_id: account.id.clone(),
        account_email: account.email.clone(),
        account_display_name: account.display_name.clone(),
        folder_path: folder_path.to_string(),
        uid,
        message_id,
        subject,
        from,
        to,
        cc,
        date,
        flags: flag_values,
        has_attachments: false,
        snippet: None,
    }
}

fn parse_body(account: &Account, folder_path: &str, uid: i64, raw: &[u8]) -> AppResult<MessageBody> {
    let parsed = MessageParser::default()
        .parse(raw)
        .ok_or_else(|| AppError::Mail("failed to parse message body".to_string()))?;

    let text_body = parsed.body_text(0).map(|body| body.to_string());
    let sanitized_html_body = parsed
        .body_html(0)
        .map(|body| sanitize_html(body.as_ref()));
    let mut attachments = Vec::new();

    for (index, part) in parsed.attachments().enumerate() {
        let filename = part
            .attachment_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("attachment-{}", index + 1));
        let mime_type = match &part.body {
            PartType::Binary(_) => "application/octet-stream",
            PartType::Text(_) => "text/plain",
            PartType::Html(_) => "text/html",
            _ => "application/octet-stream",
        };
        attachments.push(AttachmentMeta {
            account_id: account.id.clone(),
            folder_path: folder_path.to_string(),
            uid,
            filename,
            mime_type: mime_type.to_string(),
            size: Some(part.contents().len() as i64),
            part_id: (index + 1).to_string(),
        });
    }

    Ok(MessageBody {
        account_id: account.id.clone(),
        folder_path: folder_path.to_string(),
        uid,
        text_body,
        sanitized_html_body,
        fetched_at: Utc::now(),
        attachments,
    })
}

fn sanitize_html(html: &str) -> String {
    encode_safe(html).to_string()
}

fn format_addresses(addresses: &Address<'_>) -> String {
    match addresses {
        Address::List(list) => list.iter().map(format_addr).collect::<Vec<_>>().join(", "),
        Address::Group(groups) => groups
            .iter()
            .flat_map(|group| group.addresses.iter())
            .map(format_addr)
            .collect::<Vec<_>>()
            .join(", "),
    }
}

fn format_addr(addr: &Addr<'_>) -> String {
    match (&addr.name, &addr.address) {
        (Some(name), Some(address)) => format!("{name} <{address}>"),
        (None, Some(address)) => address.to_string(),
        (Some(name), None) => name.to_string(),
        (None, None) => String::new(),
    }
}

fn parse_mailbox(value: &str) -> AppResult<Mailbox> {
    value
        .trim()
        .parse()
        .map_err(|err| AppError::Validation(format!("invalid email address: {err}")))
}

fn display_folder_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn folder_role(path: &str) -> Option<String> {
    match path.to_ascii_lowercase().as_str() {
        "inbox" => Some("inbox".to_string()),
        "sent" | "sent messages" | "sent mail" => Some("sent".to_string()),
        "trash" | "deleted" | "deleted messages" => Some("trash".to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_text_message() {
        let account = Account {
            id: "a".to_string(),
            display_name: "A".to_string(),
            email: "a@example.com".to_string(),
            imap_host: "imap.example.com".to_string(),
            imap_port: 993,
            imap_security: SocketSecurity::Tls,
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 465,
            smtp_security: SocketSecurity::Tls,
            username: "a@example.com".to_string(),
            credential_key: "key".to_string(),
            is_default_sender: true,
            sync_enabled: true,
            last_error: None,
        };
        let raw = b"From: Sender <sender@example.com>\r\nTo: A <a@example.com>\r\nSubject: Hello\r\nDate: Thu, 14 May 2026 08:00:00 +0000\r\n\r\nBody text";
        let header = parse_header(&account, "INBOX", 1, &[], raw);
        assert_eq!(header.subject, "Hello");

        let body = parse_body(&account, "INBOX", 1, raw).unwrap();
        assert!(body.text_body.unwrap().contains("Body text"));
    }
}
