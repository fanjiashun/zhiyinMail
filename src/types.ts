export type SocketSecurity = "tls" | "startTls" | "plain";

export interface Account {
  id: string;
  displayName: string;
  email: string;
  imapHost: string;
  imapPort: number;
  imapSecurity: SocketSecurity;
  smtpHost: string;
  smtpPort: number;
  smtpSecurity: SocketSecurity;
  username: string;
  credentialKey: string;
  isDefaultSender: boolean;
  syncEnabled: boolean;
  lastError?: string | null;
}

export interface AccountInput {
  displayName: string;
  email: string;
  imapHost: string;
  imapPort: number;
  imapSecurity: SocketSecurity;
  smtpHost: string;
  smtpPort: number;
  smtpSecurity: SocketSecurity;
  username: string;
  password?: string;
  isDefaultSender: boolean;
  syncEnabled: boolean;
}

export interface Folder {
  accountId: string;
  path: string;
  displayName: string;
  role?: string | null;
  uidValidity?: number | null;
  lastSyncedUid?: number | null;
}

export interface MessageHeader {
  accountId: string;
  accountEmail: string;
  accountDisplayName: string;
  folderPath: string;
  uid: number;
  messageId?: string | null;
  subject: string;
  from: string;
  to: string;
  cc?: string | null;
  date: string;
  flags: string[];
  hasAttachments: boolean;
  snippet?: string | null;
}

export interface AttachmentMeta {
  accountId: string;
  folderPath: string;
  uid: number;
  filename: string;
  mimeType: string;
  size?: number | null;
  partId: string;
}

export interface MessageBody {
  accountId: string;
  folderPath: string;
  uid: number;
  textBody?: string | null;
  sanitizedHtmlBody?: string | null;
  fetchedAt: string;
  attachments: AttachmentMeta[];
}

export interface SendMessageInput {
  accountId: string;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body: string;
  attachments: string[];
}

export interface SyncReport {
  accountId: string;
  synced: number;
  error?: string | null;
}

export interface ConnectionReport {
  imapOk: boolean;
  smtpOk: boolean;
  message: string;
}
