# 知音 Mail

一个极简主义、本地优先的桌面邮箱客户端。首版目标是用统一收件箱管理多个 IMAP/SMTP 邮箱账户，同时把账号配置、邮件头缓存和最近正文缓存保存在本机。

## 功能

- 统一收件箱：聚合所有启用账户的 Inbox 邮件。
- 多账户管理：支持添加通用 IMAP/SMTP 账户。
- 本地优先：非敏感配置和邮件缓存保存到 SQLite。
- 系统凭据：邮箱密码或应用专用密码保存到系统 Keychain/Keyring。
- 邮件阅读：按需拉取正文并缓存。
- 邮件发送：支持基础 To/Cc/Bcc、主题、正文和附件发送。
- 极简 UI：玻璃拟态三栏桌面布局，支持浅色/深色切换。

## 技术栈

- Tauri 2
- React + TypeScript + Vite
- Rust backend
- SQLite via `rusqlite`
- IMAP via `imap`
- SMTP via `lettre`
- MIME parsing via `mail-parser`
- 凭据存储 via `keyring`

## 开发环境

需要安装：

- Node.js
- npm
- Rust stable toolchain
- macOS 开发环境用于 Tauri 桌面构建

安装依赖：

```bash
npm install
```

前端预览：

```bash
npm run dev
```

Tauri 开发运行：

```bash
npm run tauri dev
```

## 构建与测试

前端构建：

```bash
npm run build
```

Rust 检查：

```bash
cd src-tauri
cargo check
```

Rust 测试：

```bash
cd src-tauri
cargo test
```

## 当前范围

首版聚焦通用 IMAP/SMTP 和统一收件箱，不包含：

- OAuth 登录
- 全量离线缓存
- 统一全文搜索
- 邮件规则
- 标签同步
- 多平台打包验证

## 备注

Gmail、Outlook 等服务通常需要开启 IMAP，并使用应用专用密码或服务商允许的 IMAP/SMTP 密码方式登录。
