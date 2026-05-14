import {
  Archive,
  Briefcase,
  ChevronDown,
  Edit3,
  FileText,
  Forward,
  HelpCircle,
  Inbox,
  LayoutGrid,
  Link2,
  Mail,
  MailOpen,
  MoreHorizontal,
  Moon,
  Paperclip,
  Plus,
  RefreshCw,
  Reply,
  ReplyAll,
  Save,
  Search,
  Send,
  Settings,
  Smile,
  Sun,
  Tag,
  Trash2,
  UserRound,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api, errorMessage } from "./api";
import type {
  Account,
  AccountInput,
  ConnectionReport,
  MessageBody,
  MessageHeader,
  SendMessageInput,
} from "./types";

type View = "inbox" | "settings";
type Filter = "unified" | string;

const emptyAccountInput: AccountInput = {
  displayName: "",
  email: "",
  imapHost: "",
  imapPort: 993,
  imapSecurity: "tls",
  smtpHost: "",
  smtpPort: 465,
  smtpSecurity: "tls",
  username: "",
  password: "",
  isDefaultSender: false,
  syncEnabled: true,
};

export function App() {
  const [view, setView] = useState<View>("inbox");
  const [filter, setFilter] = useState<Filter>("unified");
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [messages, setMessages] = useState<MessageHeader[]>([]);
  const [selectedMessage, setSelectedMessage] = useState<MessageHeader | null>(null);
  const [messageBody, setMessageBody] = useState<MessageBody | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [isComposeOpen, setComposeOpen] = useState(false);
  const [isAccountOpen, setAccountOpen] = useState(false);
  const [isDarkMode, setDarkMode] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);

  const defaultAccount = useMemo(
    () => accounts.find((account) => account.isDefaultSender) ?? accounts[0],
    [accounts],
  );
  const activeAccount = filter === "unified" ? defaultAccount : accounts.find((account) => account.id === filter);

  const filteredMessages = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return messages;
    return messages.filter((message) =>
      [message.subject, message.from, message.accountEmail, message.snippet]
        .join(" ")
        .toLowerCase()
        .includes(query),
    );
  }, [messages, searchQuery]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", isDarkMode);
  }, [isDarkMode]);

  useEffect(() => {
    loadAccounts()
      .then(() => loadInbox("unified"))
      .then(() => api.syncAllInboxes())
      .then(() => loadInbox("unified"))
      .then(loadAccounts)
      .catch((error) => setNotice(errorMessage(error)));
  }, []);

  async function loadAccounts() {
    setAccounts(await api.listAccounts());
  }

  async function loadInbox(nextFilter = filter) {
    const accountId = nextFilter === "unified" ? undefined : nextFilter;
    const nextMessages = await api.listUnifiedInbox(accountId);
    setMessages(nextMessages);
    if (selectedMessage && !nextMessages.some((message) => sameMessage(message, selectedMessage))) {
      setSelectedMessage(null);
      setMessageBody(null);
    }
  }

  async function syncInbox() {
    setSyncing(true);
    setNotice(null);
    try {
      const reports = filter === "unified"
        ? await api.syncAllInboxes()
        : [await api.syncAccountInbox(filter)];
      const synced = reports.reduce((sum, report) => sum + report.synced, 0);
      const failed = reports.filter((report) => report.error).length;
      setNotice(failed ? `${synced} synced. ${failed} account failed.` : `${synced} synced.`);
      await loadInbox();
      await loadAccounts();
    } catch (error) {
      setNotice(errorMessage(error));
    } finally {
      setSyncing(false);
    }
  }

  async function selectMessage(message: MessageHeader) {
    setSelectedMessage(message);
    setMessageBody(null);
    try {
      setMessageBody(await api.getMessageBody(message.accountId, message.folderPath, message.uid));
    } catch (error) {
      setNotice(errorMessage(error));
    }
  }

  async function changeFilter(nextFilter: Filter) {
    setFilter(nextFilter);
    setView("inbox");
    setSelectedMessage(null);
    setMessageBody(null);
    await loadInbox(nextFilter);
  }

  return (
    <div className="app-shell">
      <Sidebar
        accounts={accounts}
        activeView={view}
        activeFilter={filter}
        messageCount={messages.length}
        defaultAccount={defaultAccount}
        onViewChange={setView}
        onFilterChange={changeFilter}
        onCompose={() => setComposeOpen(true)}
        onAddAccount={() => setAccountOpen(true)}
      />

      <main className="main-surface">
        <TopBar
          title={view === "inbox" ? "Unified Inbox" : "Account Settings"}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          onRefresh={syncInbox}
          refreshing={syncing}
          selectedMessage={selectedMessage}
          onDelete={async () => {
            if (!selectedMessage) return;
            await api.deleteMessage(selectedMessage.accountId, selectedMessage.folderPath, selectedMessage.uid);
            setSelectedMessage(null);
            setMessageBody(null);
            await loadInbox();
          }}
        />

        {notice && (
          <div className="toast">
            <span>{notice}</span>
            <button onClick={() => setNotice(null)} aria-label="Dismiss notice">
              <X size={14} />
            </button>
          </div>
        )}

        <div className="content-row">
          {view === "inbox" ? (
            <>
              <MessageList
                messages={filteredMessages}
                selectedMessage={selectedMessage}
                onSelect={selectMessage}
              />
              <ReadingPane
                message={selectedMessage}
                body={messageBody}
                onReply={() => setComposeOpen(true)}
              />
            </>
          ) : (
            <SettingsView
              accounts={accounts}
              onAddAccount={() => setAccountOpen(true)}
              onRefresh={syncInbox}
            />
          )}
        </div>

        <button
          className="theme-toggle"
          onClick={() => setDarkMode((current) => !current)}
          aria-label="Toggle theme"
          title="Toggle theme"
        >
          {isDarkMode ? <Sun size={19} /> : <Moon size={19} />}
        </button>
      </main>

      {isAccountOpen && (
        <AccountModal
          onClose={() => setAccountOpen(false)}
          onSaved={async () => {
            setAccountOpen(false);
            await loadAccounts();
            await loadInbox();
          }}
        />
      )}

      {isComposeOpen && defaultAccount && (
        <ComposeModal
          accounts={accounts}
          defaultAccount={selectedMessage
            ? accounts.find((account) => account.id === selectedMessage.accountId) ?? defaultAccount
            : activeAccount ?? defaultAccount}
          selectedMessage={selectedMessage}
          onClose={() => setComposeOpen(false)}
          onSent={() => {
            setComposeOpen(false);
            setNotice("Message sent.");
          }}
        />
      )}
    </div>
  );
}

function Sidebar({
  accounts,
  activeView,
  activeFilter,
  messageCount,
  defaultAccount,
  onViewChange,
  onFilterChange,
  onCompose,
  onAddAccount,
}: {
  accounts: Account[];
  activeView: View;
  activeFilter: Filter;
  messageCount: number;
  defaultAccount?: Account;
  onViewChange: (view: View) => void;
  onFilterChange: (filter: Filter) => void;
  onCompose: () => void;
  onAddAccount: () => void;
}) {
  const accountInitial = (defaultAccount?.displayName || "U").charAt(0).toUpperCase();

  return (
    <aside className="sidebar-glass">
      <button className="account-switcher" onClick={onAddAccount}>
        <span className="account-dot">{accountInitial}</span>
        <span className="account-copy">
          <strong>{defaultAccount?.displayName ?? "No account"}</strong>
          <small>{defaultAccount?.email ?? "Add an IMAP account"}</small>
        </span>
        <ChevronDown size={14} />
      </button>

      <button className="compose-button" onClick={onCompose} disabled={!defaultAccount}>
        <Edit3 size={16} />
        Compose
      </button>

      <nav className="sidebar-nav" aria-label="Mailbox navigation">
        <SidebarButton
          active={activeView === "inbox" && activeFilter === "unified"}
          icon={LayoutGrid}
          label="Unified Inbox"
          count={messageCount}
          onClick={() => onFilterChange("unified")}
        />
        <SidebarButton
          active={false}
          icon={Inbox}
          label="Inbox"
          onClick={() => onFilterChange("unified")}
        />
        <SidebarButton icon={Send} label="Sent" disabled />
        <SidebarButton icon={FileText} label="Drafts" disabled />
        <SidebarButton icon={Archive} label="Archive" disabled />
        <SidebarButton icon={Trash2} label="Trash" disabled />

        <div className="nav-title">Accounts</div>
        {accounts.map((account) => (
          <SidebarButton
            key={account.id}
            active={activeView === "inbox" && activeFilter === account.id}
            icon={UserRound}
            label={account.displayName}
            sublabel={account.lastError ?? account.email}
            onClick={() => onFilterChange(account.id)}
          />
        ))}

        <div className="nav-title">Labels</div>
        <button className="label-row" disabled>
          <span className="label-dot blue" />
          Work
        </button>
        <button className="label-row" disabled>
          <span className="label-dot amber" />
          Personal
        </button>
      </nav>

      <div className="sidebar-footer">
        <SidebarButton
          active={activeView === "settings"}
          icon={Settings}
          label="Settings"
          onClick={() => onViewChange("settings")}
        />
        <SidebarButton icon={HelpCircle} label="Help" disabled />
      </div>
    </aside>
  );
}

function SidebarButton({
  active = false,
  icon: Icon,
  label,
  sublabel,
  count,
  disabled,
  onClick,
}: {
  active?: boolean;
  icon: typeof Inbox;
  label: string;
  sublabel?: string;
  count?: number;
  disabled?: boolean;
  onClick?: () => void;
}) {
  return (
    <button className={`nav-row ${active ? "active" : ""}`} disabled={disabled} onClick={onClick}>
      <Icon size={16} />
      <span className="nav-copy">
        <strong>{label}</strong>
        {sublabel && <small>{sublabel}</small>}
      </span>
      {typeof count === "number" && <span className="nav-count">{count}</span>}
    </button>
  );
}

function TopBar({
  title,
  searchQuery,
  onSearchChange,
  onRefresh,
  refreshing,
  selectedMessage,
  onDelete,
}: {
  title: string;
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onRefresh: () => void;
  refreshing: boolean;
  selectedMessage: MessageHeader | null;
  onDelete: () => void;
}) {
  return (
    <header className="topbar">
      <div className="search-wrap">
        <Search size={16} />
        <input
          value={searchQuery}
          onChange={(event) => onSearchChange(event.target.value)}
          placeholder="Search..."
        />
      </div>

      <div className="topbar-title">{title}</div>

      <div className="top-actions">
        <button title="Refresh" onClick={onRefresh} className="icon-action">
          <RefreshCw size={18} className={refreshing ? "spin" : ""} />
        </button>
        <button title="Archive" className="icon-action" disabled={!selectedMessage}>
          <Archive size={18} />
        </button>
        <button title="Delete" className="icon-action danger" disabled={!selectedMessage} onClick={onDelete}>
          <Trash2 size={18} />
        </button>
        <span className="divider" />
        <button title="Mark as unread" className="icon-action" disabled={!selectedMessage}>
          <MailOpen size={18} />
        </button>
        <button title="Label" className="icon-action" disabled={!selectedMessage}>
          <Tag size={18} />
        </button>
      </div>
    </header>
  );
}

function MessageList({
  messages,
  selectedMessage,
  onSelect,
}: {
  messages: MessageHeader[];
  selectedMessage: MessageHeader | null;
  onSelect: (message: MessageHeader) => void;
}) {
  return (
    <section className="message-list-pane" aria-label="Message list">
      <div className="message-list-scroll">
        {messages.length === 0 ? (
          <div className="soft-empty">No messages found</div>
        ) : messages.map((message) => {
          const selected = Boolean(selectedMessage && sameMessage(message, selectedMessage));
          const unread = !message.flags.some((flag) => flag.toLowerCase().includes("seen"));
          return (
            <button
              key={`${message.accountId}:${message.folderPath}:${message.uid}`}
              className={`mail-row ${selected ? "selected" : ""}`}
              onClick={() => onSelect(message)}
            >
              {selected && <span className="selected-bar" />}
              <span className="mail-row-top">
                <strong className={unread ? "unread" : ""}>{message.from || "Unknown sender"}</strong>
                <span className="mail-time">{formatShortDate(message.date)}</span>
              </span>
              <span className={`mail-subject ${unread ? "unread" : ""}`}>{message.subject}</span>
              <span className="mail-snippet">{message.snippet || message.accountDisplayName}</span>
              <span className="mail-footer">
                <span className="label-dot blue" />
                <span>{message.accountDisplayName}</span>
                {message.hasAttachments && <Paperclip size={13} />}
              </span>
            </button>
          );
        })}
      </div>
    </section>
  );
}

function ReadingPane({
  message,
  body,
  onReply,
}: {
  message: MessageHeader | null;
  body: MessageBody | null;
  onReply: () => void;
}) {
  if (!message) {
    return (
      <section className="reading-empty">
        <MoreHorizontal size={48} />
        <p>Select an email to view</p>
      </section>
    );
  }

  return (
    <section className="reading-pane">
      <article className="reading-scroll">
        <header className="reading-header">
          <div className="reading-title-line">
            <h1>{message.subject}</h1>
            <div className="reading-actions">
              <button title="Reply" onClick={onReply}><Reply size={18} /></button>
              <button title="Reply all"><ReplyAll size={18} /></button>
              <button title="Forward"><Forward size={18} /></button>
              <span className="divider" />
              <button title="More"><MoreHorizontal size={18} /></button>
            </div>
          </div>
          <div className="sender-row">
            <span className="sender-avatar">{senderInitial(message.from)}</span>
            <span className="sender-copy">
              <strong>{message.from || "Unknown sender"}</strong>
              <small>To: {message.to || message.accountEmail}</small>
            </span>
            <time>{formatLongDate(message.date)}</time>
          </div>
        </header>

        <div className="reader-body">
          {!body && <div className="soft-empty">Loading message body...</div>}
          {body?.textBody && <pre>{body.textBody}</pre>}
          {!body?.textBody && body?.sanitizedHtmlBody && <pre>{body.sanitizedHtmlBody}</pre>}
          {body && !body.textBody && !body.sanitizedHtmlBody && (
            <div className="soft-empty">No readable body found.</div>
          )}
        </div>

        {body && body.attachments.length > 0 && (
          <div className="attachment-grid">
            {body.attachments.map((attachment) => (
              <button key={attachment.partId} className="attachment-card">
                <Paperclip size={16} />
                <span>{attachment.filename}</span>
              </button>
            ))}
          </div>
        )}
      </article>
    </section>
  );
}

function SettingsView({
  accounts,
  onAddAccount,
  onRefresh,
}: {
  accounts: Account[];
  onAddAccount: () => void;
  onRefresh: () => void;
}) {
  const activeCount = accounts.filter((account) => account.syncEnabled).length;
  return (
    <section className="settings-view">
      <div className="settings-inner">
        <header className="settings-hero">
          <h1>Account Settings</h1>
          <p>Manage connected IMAP/SMTP accounts and local sync preferences.</p>
        </header>

        <section className="profile-card">
          <span className="profile-avatar"><Briefcase size={24} /></span>
          <span>
            <h2>{accounts[0]?.displayName ?? "Unified Mail"}</h2>
            <p>{accounts[0]?.email ?? "Local-first desktop mail"}</p>
          </span>
          <button onClick={onRefresh}>Refresh mail</button>
        </section>

        <section>
          <div className="settings-section-title">
            <h3>Connected Accounts</h3>
            <span>{activeCount} Active</span>
          </div>
          <div className="settings-list">
            {accounts.map((account) => (
              <div className="settings-account" key={account.id}>
                <span className={`settings-account-icon ${account.syncEnabled ? "active" : ""}`}>
                  <Mail size={20} />
                </span>
                <span className="settings-account-copy">
                  <strong>{account.displayName}</strong>
                  <small>{account.email}</small>
                </span>
                <span className="account-type">IMAP</span>
                <span className={`toggle ${account.syncEnabled ? "on" : ""}`} />
                <button><MoreHorizontal size={18} /></button>
              </div>
            ))}
            {accounts.length === 0 && <div className="soft-empty">No accounts connected.</div>}
          </div>
        </section>

        <section>
          <h3 className="provider-title">Add Provider</h3>
          <div className="provider-grid">
            <button onClick={onAddAccount}>
              <Plus size={20} />
              <span><strong>Other IMAP Account</strong><small>Manual configuration for custom domains</small></span>
            </button>
          </div>
        </section>
      </div>
    </section>
  );
}

function AccountModal({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const [input, setInput] = useState<AccountInput>(emptyAccountInput);
  const [testing, setTesting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [report, setReport] = useState<ConnectionReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  function update<K extends keyof AccountInput>(key: K, value: AccountInput[K]) {
    setInput((current) => ({ ...current, [key]: value }));
  }

  async function testConnection() {
    setTesting(true);
    setError(null);
    try {
      setReport(await api.testAccountConnection(input));
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setTesting(false);
    }
  }

  async function save() {
    setSaving(true);
    setError(null);
    try {
      await api.addAccount(input);
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="modal-layer" role="dialog" aria-modal="true">
      <button className="modal-backdrop" onClick={onClose} aria-label="Close account dialog" />
      <form className="account-modal modal-window" onSubmit={(event) => { event.preventDefault(); save(); }}>
        <ModalHeader title="Add IMAP Account" onClose={onClose} />
        <div className="form-grid">
          <label htmlFor="account-display-name">Display name<input id="account-display-name" value={input.displayName} onChange={(e) => update("displayName", e.target.value)} /></label>
          <label htmlFor="account-email">Email<input id="account-email" value={input.email} onChange={(e) => update("email", e.target.value)} /></label>
          <label htmlFor="account-username">Username<input id="account-username" value={input.username} onChange={(e) => update("username", e.target.value)} /></label>
          <label htmlFor="account-password">App password<input id="account-password" type="password" value={input.password} onChange={(e) => update("password", e.target.value)} /></label>
          <label htmlFor="account-imap-host">IMAP host<input id="account-imap-host" value={input.imapHost} onChange={(e) => update("imapHost", e.target.value)} /></label>
          <label htmlFor="account-imap-port">IMAP port<input id="account-imap-port" type="number" value={input.imapPort} onChange={(e) => update("imapPort", Number(e.target.value))} /></label>
          <label htmlFor="account-smtp-host">SMTP host<input id="account-smtp-host" value={input.smtpHost} onChange={(e) => update("smtpHost", e.target.value)} /></label>
          <label htmlFor="account-smtp-port">SMTP port<input id="account-smtp-port" type="number" value={input.smtpPort} onChange={(e) => update("smtpPort", Number(e.target.value))} /></label>
        </div>
        <label className="checkbox-line">
          <input
            type="checkbox"
            checked={input.isDefaultSender}
            onChange={(event) => update("isDefaultSender", event.target.checked)}
          />
          Use as default sender
        </label>
        {report && <div className="success-note">{report.message}</div>}
        {error && <div className="error-note">{error}</div>}
        <footer className="modal-actions">
          <button type="button" onClick={testConnection} disabled={testing}>
            <RefreshCw size={16} className={testing ? "spin" : ""} />
            Test
          </button>
          <button type="submit" className="primary" disabled={saving}>
            <Plus size={16} />
            Save account
          </button>
        </footer>
      </form>
    </div>
  );
}

function ComposeModal({
  accounts,
  defaultAccount,
  selectedMessage,
  onClose,
  onSent,
}: {
  accounts: Account[];
  defaultAccount: Account;
  selectedMessage: MessageHeader | null;
  onClose: () => void;
  onSent: () => void;
}) {
  const [input, setInput] = useState<SendMessageInput>({
    accountId: defaultAccount.id,
    to: selectedMessage ? [extractEmail(selectedMessage.from)] : [],
    cc: [],
    bcc: [],
    subject: selectedMessage ? `Re: ${selectedMessage.subject.replace(/^Re:\s*/i, "")}` : "",
    body: "",
    attachments: [],
  });
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);

  async function send() {
    setSending(true);
    setError(null);
    try {
      await api.sendMessage(input);
      onSent();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSending(false);
    }
  }

  return (
    <div className="modal-layer" role="dialog" aria-modal="true">
      <button className="modal-backdrop" onClick={onClose} aria-label="Close compose dialog" />
      <form className="compose-modal modal-window" onSubmit={(event) => { event.preventDefault(); send(); }}>
        <ModalHeader title="New Message" onClose={onClose} />
        <div className="compose-fields">
          <label>
            <span>From:</span>
            <select
              value={input.accountId}
              onChange={(event) => setInput((current) => ({ ...current, accountId: event.target.value }))}
            >
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>{account.displayName} ({account.email})</option>
              ))}
            </select>
          </label>
          <label>
            <span>To:</span>
            <input
              value={input.to.join(", ")}
              onChange={(event) => setInput((current) => ({ ...current, to: splitAddresses(event.target.value) }))}
            />
            <em>Cc</em><em>Bcc</em>
          </label>
          <label>
            <span>Subject:</span>
            <input
              value={input.subject}
              placeholder="Enter subject here..."
              onChange={(event) => setInput((current) => ({ ...current, subject: event.target.value }))}
            />
          </label>
          <textarea
            placeholder="Write your message..."
            value={input.body}
            onChange={(event) => setInput((current) => ({ ...current, body: event.target.value }))}
          />
        </div>
        {error && <div className="error-note">{error}</div>}
        <footer className="compose-footer">
          <div className="format-actions">
            <button type="button"><Smile size={18} /></button>
            <span className="divider" />
            <button type="button"><Paperclip size={18} /></button>
            <button type="button"><Link2 size={18} /></button>
            <button type="button"><Edit3 size={18} /></button>
          </div>
          <div className="send-actions">
            <button type="button" onClick={onClose}><Save size={16} /> Save Draft</button>
            <button type="button" className="trash"><Trash2 size={18} /></button>
            <button type="submit" className="primary" disabled={sending}>
              Send
              <Send size={14} />
            </button>
          </div>
        </footer>
      </form>
    </div>
  );
}

function ModalHeader({ title, onClose }: { title: string; onClose: () => void }) {
  return (
    <header className="modal-header">
      <span className="traffic-lights">
        <button type="button" className="red" onClick={onClose} aria-label="Close" />
        <button type="button" className="yellow" aria-label="Minimize" />
        <button type="button" className="green" aria-label="Maximize" />
      </span>
      <strong>{title}</strong>
      <button type="button" onClick={onClose} aria-label="Close">
        <X size={16} />
      </button>
    </header>
  );
}

function sameMessage(left: MessageHeader, right: MessageHeader) {
  return left.accountId === right.accountId && left.folderPath === right.folderPath && left.uid === right.uid;
}

function formatShortDate(value: string) {
  return new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" }).format(new Date(value));
}

function formatLongDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(value));
}

function senderInitial(value: string) {
  return (value.trim().replace(/^["<]/, "").charAt(0) || "M").toUpperCase();
}

function splitAddresses(value: string) {
  return value.split(",").map((part) => part.trim()).filter(Boolean);
}

function extractEmail(value: string) {
  const match = value.match(/<([^>]+)>/);
  return match?.[1] ?? value;
}
