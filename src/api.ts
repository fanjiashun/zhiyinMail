import { invoke } from "@tauri-apps/api/core";
import type {
  Account,
  AccountInput,
  ConnectionReport,
  Folder,
  MessageBody,
  MessageHeader,
  SendMessageInput,
  SyncReport,
} from "./types";

const isTauri = "__TAURI_INTERNALS__" in window;
const mockAccounts: Account[] = [];

export const api = {
  addAccount(input: AccountInput) {
    if (!isTauri) {
      const id = crypto.randomUUID();
      const account: Account = {
        id,
        displayName: input.displayName,
        email: input.email,
        imapHost: input.imapHost,
        imapPort: input.imapPort,
        imapSecurity: input.imapSecurity,
        smtpHost: input.smtpHost,
        smtpPort: input.smtpPort,
        smtpSecurity: input.smtpSecurity,
        username: input.username,
        credentialKey: `mock:${id}`,
        isDefaultSender: input.isDefaultSender || mockAccounts.length === 0,
        syncEnabled: input.syncEnabled,
        lastError: null,
      };
      if (account.isDefaultSender) {
        mockAccounts.forEach((item) => {
          item.isDefaultSender = false;
        });
      }
      mockAccounts.push(account);
      return Promise.resolve(account);
    }
    return invoke<Account>("add_account", { input });
  },
  updateAccount(id: string, input: AccountInput) {
    if (!isTauri) {
      const index = mockAccounts.findIndex((account) => account.id === id);
      if (index === -1) return Promise.reject(new Error("account not found"));
      mockAccounts[index] = {
        ...mockAccounts[index],
        displayName: input.displayName,
        email: input.email,
        imapHost: input.imapHost,
        imapPort: input.imapPort,
        imapSecurity: input.imapSecurity,
        smtpHost: input.smtpHost,
        smtpPort: input.smtpPort,
        smtpSecurity: input.smtpSecurity,
        username: input.username,
        isDefaultSender: input.isDefaultSender,
        syncEnabled: input.syncEnabled,
      };
      return Promise.resolve(mockAccounts[index]);
    }
    return invoke<Account>("update_account", { id, input });
  },
  removeAccount(id: string) {
    if (!isTauri) {
      const index = mockAccounts.findIndex((account) => account.id === id);
      if (index >= 0) mockAccounts.splice(index, 1);
      return Promise.resolve();
    }
    return invoke<void>("remove_account", { id });
  },
  testAccountConnection(input: AccountInput) {
    if (!isTauri) {
      const ok = Boolean(input.imapHost && input.smtpHost && input.username && input.password);
      return Promise.resolve({
        imapOk: ok,
        smtpOk: ok,
        message: ok
          ? "Preview connection check passed."
          : "Fill IMAP, SMTP, username, and app password.",
      });
    }
    return invoke<ConnectionReport>("test_account_connection", { input });
  },
  listAccounts() {
    if (!isTauri) return Promise.resolve([...mockAccounts]);
    return invoke<Account[]>("list_accounts");
  },
  listFolders(accountId: string) {
    if (!isTauri) {
      return Promise.resolve([{ accountId, path: "INBOX", displayName: "Inbox", role: "inbox" }]);
    }
    return invoke<Folder[]>("list_folders", { accountId });
  },
  syncAccountInbox(accountId: string) {
    if (!isTauri) return Promise.resolve({ accountId, synced: 0, error: null });
    return invoke<SyncReport>("sync_account_inbox", { accountId });
  },
  syncAllInboxes() {
    if (!isTauri) return Promise.resolve<SyncReport[]>([]);
    return invoke<SyncReport[]>("sync_all_inboxes");
  },
  listUnifiedInbox(accountId?: string) {
    if (!isTauri) {
      const account = accountId ? mockAccounts.find((item) => item.id === accountId) : mockAccounts[0];
      if (!account) return Promise.resolve([]);
      return Promise.resolve<MessageHeader[]>([
        {
          accountId: account.id,
          accountEmail: account.email,
          accountDisplayName: account.displayName,
          folderPath: "INBOX",
          uid: 1,
          messageId: "preview-message",
          subject: "Welcome to Unified Mail",
          from: "Preview Sender <sender@example.com>",
          to: account.email,
          cc: null,
          date: new Date().toISOString(),
          flags: [],
          hasAttachments: false,
          snippet: "This preview message is only shown outside the Tauri desktop shell.",
        },
      ]);
    }
    return invoke<MessageHeader[]>("list_unified_inbox", { accountId });
  },
  listAccountMessages(accountId: string, folderPath?: string) {
    if (!isTauri) return this.listUnifiedInbox(accountId);
    return invoke<MessageHeader[]>("list_account_messages", {
      accountId,
      folderPath,
    });
  },
  getMessageBody(accountId: string, folderPath: string, uid: number) {
    if (!isTauri) {
      return Promise.resolve({
        accountId,
        folderPath,
        uid,
        textBody:
          "This is the browser preview path. In the Tauri app, this body is fetched from IMAP and cached locally.",
        sanitizedHtmlBody: null,
        fetchedAt: new Date().toISOString(),
        attachments: [],
      });
    }
    return invoke<MessageBody>("get_message_body", { accountId, folderPath, uid });
  },
  downloadAttachment(
    accountId: string,
    folderPath: string,
    uid: number,
    partId: string,
    outputPath: string,
  ) {
    if (!isTauri) return Promise.resolve();
    return invoke<void>("download_attachment", {
      accountId,
      folderPath,
      uid,
      partId,
      outputPath,
    });
  },
  sendMessage(input: SendMessageInput) {
    if (!isTauri) {
      if (!input.to.length) return Promise.reject(new Error("at least one recipient is required"));
      return Promise.resolve();
    }
    return invoke<void>("send_message", { input });
  },
  markRead(accountId: string, folderPath: string, uid: number) {
    if (!isTauri) return Promise.resolve();
    return invoke<void>("mark_read", { accountId, folderPath, uid });
  },
  markUnread(accountId: string, folderPath: string, uid: number) {
    if (!isTauri) return Promise.resolve();
    return invoke<void>("mark_unread", { accountId, folderPath, uid });
  },
  deleteMessage(accountId: string, folderPath: string, uid: number) {
    if (!isTauri) return Promise.resolve();
    return invoke<void>("delete_message", { accountId, folderPath, uid });
  },
};

export function errorMessage(error: unknown) {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as { message: unknown }).message);
  }
  return "Something went wrong.";
}
