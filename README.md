# GitKord

**Discord Git Bot** — a live pull request companion for Discord, powered by Rust.

[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Discord](https://img.shields.io/badge/discord-slash--commands-5865F2.svg)](https://discord.com)

GitKord listens to GitHub webhook events and maintains a single, always-up-to-date Discord message for each open pull request. Reviewer status, commit activity, and PR state stay in sync across every subscribed channel — no manual refreshing, no channel clutter.

> **Status:** Active development — see [Roadmap](#roadmap) for what's landed and what's next.

---

## Features

- **Multi-channel subscriptions** — subscribe the same repo in multiple channels of the same server; each channel gets its own PR message and audit thread.
- **One message per PR** — title, status emoji, author, and branch update in place as events arrive.
- **Audit thread** — every event (opened, review, assign, close) is appended to a thread on the main message for full traceability.
- **Auto webhook registration** — `/subscribe` registers the GitHub webhook automatically. No manual setup in repo settings.
- **Reviewer assignment** — `/assign` and `/unassign` work with GitHub usernames or Discord mentions. Run inside a PR thread and the repo and PR number are inferred automatically.
- **Discord to GitHub linking** — `/link` verifies the GitHub account exists before saving the mapping. Linked reviewers display as Discord mentions in the automated messages.
- **Self-assignment guard** — the bot rejects review requests where the requester and reviewer are the same person.
- **Commit-push aware** — pushes to an open PR trigger a `pull_request synchronize` event that updates the PR message in every subscribed channel.
- **Slash commands** — all ephemeral, no channel noise.
- **Postgres persistence** — PR message IDs, thread IDs, subscriptions, and user links survive restarts.
- **Health endpoint** — `GET /healthz` for uptime monitors and Railway health checks.

---

## How It Works

```
GitHub webhook (pull_request, pull_request_review, issue_comment, push)
        |
        v
┌──────────────────────┐
│   Axum HTTP server   │  <- verifies HMAC-SHA256 signature
│   /github/webhook    │
└──────────┬───────────┘
           |  deserialized event
           v
┌──────────────────────┐
│    Event handler     │  <- translates event into a Discord action
└──────────┬───────────┘
           |
           v
┌──────────────────────┐
│  Discord (Serenity)  │  <- edits main message, appends to audit thread
└──────────────────────┘
```

1. A pull request is opened, reviewed, or merged on GitHub.
2. GitHub sends a signed webhook payload to GitKord.
3. GitKord updates the pinned message in every subscribed channel and appends an entry to the audit thread.
4. Users interact through slash commands — everything stays in one place.

---

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.91+
- A [Discord application](https://discord.com/developers/applications) with a bot token
- A GitHub fine-grained PAT with **Pull requests** and **Webhooks** read/write permissions
- A publicly reachable URL (Railway in production, ngrok in development)

### Environment Variables

| Variable                | Required | Description                                                         |
|-------------------------|----------|---------------------------------------------------------------------|
| `DISCORD_TOKEN`         | Yes      | Discord bot token                                                   |
| `GITHUB_WEBHOOK_SECRET` | Yes      | HMAC secret for verifying webhook payloads (`openssl rand -hex 32`) |
| `GITHUB_TOKEN`          | Yes      | GitHub PAT (Pull requests + Webhooks read/write)                    |
| `PUBLIC_DOMAIN`         | Yes      | Public domain GitKord is reachable at (no trailing slashes)         |
| `DATABASE_URL`          | No       | Postgres connection url                                             |
| `RUST_LOG`              | No       | Log level: `trace`, `debug`, `info`, `warn`                         |
| `PORT`                  | No       | HTTP listen port, defaults to `3000`                                |

### Build and Run

```bash
git clone https://github.com/kevinlizarazu/gitkord.git
cd gitkord

cargo build --release

DISCORD_TOKEN=...             \
GITHUB_WEBHOOK_SECRET=...     \
GITHUB_TOKEN=...              \
PUBLIC_DOMAIN=...  \
./target/release/GitKord
```

### Invite the Bot

Generate an OAuth2 URL with these scopes and permissions:

**Scopes:** `bot`, `applications.commands`

**Permissions:** Send Messages, Create Public Threads, Send Messages in Threads, Manage Messages, Use Slash Commands

### Local Development

Use [ngrok](https://ngrok.com) to expose your local port:

```bash
ngrok http 3001   # use a port that does not conflict with other local servers
export PORT=3001
export PUBLIC_DOMAIN=your-ngrok-url.ngrok-free.app
cargo run
```

---

## Slash Commands

| Command      | Description                                                                 |
|--------------|-----------------------------------------------------------------------------|
| `/subscribe` | Subscribe this channel to PR updates for a repo. Registers the webhook too. |
| `/unsubscribe` | Stop receiving PR updates for a repo in this channel.                     |
| `/link`      | Link your Discord account to a GitHub username (verifies the account exists). |
| `/unlink`    | Remove your Discord to GitHub link.                                         |
| `/assign`    | Request a review. Run inside a PR thread to skip `repo` and `pr` options.  |
| `/unassign`  | Remove a review request. Same thread-aware behavior as `/assign`.           |
| `/health`    | Check if GitKord is running.                                                |

---

## Status Emojis

| Emoji | Meaning         |
|-------|-----------------|
| 🟢    | Open            |
| 🔴    | Closed          |
| 🟣    | Merged          |

Review verdicts in the audit thread use separate emojis: ✅ approved, 🛑 changes requested, 💬 commented, 🟡 pending.

---

## Roadmap

### v0.1 — Foundation ✅
- Axum HTTP server with `/healthz` and webhook endpoint
- Serenity Discord client with gateway connection
- Config loaded from environment variables with fast-fail validation
- Unified `AppError` with `IntoResponse` for automatic HTTP status mapping
- `rustfmt` and `clippy` configured for stable linting

### v0.2 — Core loop ✅
- HMAC-SHA256 webhook signature verification
- `pull_request` payload deserialization
- Discord message creation, in-place updates, and audit thread per PR
- Postgres persistence for PR message IDs and thread IDs
- `pull_request_review` events posted to audit thread

### v0.3 — Subscriptions and commands ✅
- Dynamic subscription store (any channel can subscribe to any repo)
- `/subscribe` auto-registers the GitHub webhook via API
- `/link` and `/unlink` with GitHub user verification
- `/assign` and `/unassign` with Discord mention resolution and self-assignment guard
- Thread-aware context inference for assign/unassign
- `CommandContext` struct grouping slash command dependencies
- `UserLinkStore` threaded into webhook handler for reviewer Discord mention resolution
- Multi-channel subscriptions — same repo in multiple channels of the same guild
- `SubscriptionStore::get_by_guild` returns `Vec<Subscription>` for per-channel delete and list

### v0.4 — Deployment and testing
- Railway deployment with persistent URL
- Test suite (unit tests for signature verification, store traits, formatters)
- Message and embed redesign
- Commit-push triggers PR message update via `pull_request synchronize`

### v0.5 — Polish
- Remind button with DM delivery and cooldown store
- Review approval status displayed on the main PR message
- Multi-repo support improvements
- GitHub OAuth for automatic Discord to GitHub linking

---

## Architecture

```
src/
├── main.rs              # Entrypoint — spawns Axum and Serenity concurrently
├── config.rs            # Environment variable loading, single source of truth
├── error.rs             # AppError with IntoResponse for Axum handlers
├── github/
│   ├── api.rs           # Octocrab helpers (verify user, register webhook, assign reviewer, fetch PR data)
│   ├── client.rs        # GitHub HTTP client construction
│   ├── context.rs       # WebhookState — shared deps for webhook event handlers
│   ├── models.rs        # Domain models (PrMessageData, ReviewSummary, CheckSummary)
│   ├── payloads.rs      # Webhook payload structs (PullRequest, Review, PushPayload, etc.)
│   └── webhook.rs       # Axum route, HMAC verification, event dispatch
├── discord/
│   ├── context.rs       # AppState — shared deps for slash command handlers
│   ├── models.rs        # Domain models (ReadyHandler, PostedPullRequest, ReviewerRequest)
│   ├── client.rs        # Serenity client construction and Http handle extraction
│   ├── bot.rs           # Serenity event handler (slash command dispatch on interaction)
│   ├── commands/
│   │   ├── mod.rs       # Slash command registration and dispatch
│   │   ├── health.rs    # /health handler
│   │   ├── subscription.rs  # /subscribe and /unsubscribe handlers
│   │   ├── user_link.rs # /link and /unlink handlers
│   │   ├── reviewer.rs  # /assign and /unassign handlers
│   │   └── shared.rs    # Ephemeral reply and option parsing helpers
│   └── messages/
│       ├── mod.rs       # Public message API (post, update, audit)
│       ├── renderer.rs  # Embed construction and formatting
│       ├── transport.rs # Serenity HTTP helpers (send, edit, create thread)
│       └── audit.rs     # Audit thread helpers for review/state events
└── db/
    ├── mod.rs           # Re-exports (PrChannelMessageStore, SubscriptionStore, UserLinkStore)
    ├── models.rs        # Data models (PrChannelMessage, Subscription, UserLink)
    ├── traits.rs        # Store trait abstractions
    └── postgres/
        ├── mod.rs       # PostgresStore re-exports
        ├── schema.rs    # Connection and table creation
        ├── pr_channel_messages.rs  # PrChannelMessageStore impl
        ├── subscriptions.rs       # SubscriptionStore impl
        └── user_links.rs          # UserLinkStore impl
```

### Key Dependencies

| Crate       | Purpose                            |
|-------------|------------------------------------|
| `axum`      | HTTP server and webhook endpoint   |
| `serenity`  | Discord gateway and slash commands |
| `octocrab`  | GitHub REST API client             |
| `sqlx`      | Postgres persistence               |
| `tokio`     | Async runtime                      |
| `tracing`   | Structured logging                 |
| `hmac`/`sha2` | Webhook signature verification     |
| `indexmap`    | Ordered reviewer tracking          |

### Design Principles

- **Single responsibility** — each module does one thing.
- **Open/closed** — stores sit behind traits; swap the backend without touching handlers.
- **Interface segregation** — separate focused traits for PR messages, subscriptions, and user links.
- **Dependency inversion** — handlers depend on trait interfaces, not concrete types.
- **CommandContext** — slash command dependencies grouped in one struct so dispatch stays readable.

---

## Development

```bash
# Run
cargo run

# Run with hot reload (requires cargo-watch)
cargo watch -x run

# Lint
cargo clippy -- -D warnings

# Format check
cargo fmt -- --check

# Tests
cargo test
```

> Set `RUST_LOG=debug` to see full event payloads during webhook development.

---

## Contributing

Run `cargo fmt` and `cargo clippy -- -D warnings` before opening a pull request.

---

## License

MIT © Kevin Lizarazu