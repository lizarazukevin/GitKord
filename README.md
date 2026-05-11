# DiGiBot

**Discord Git Bot** — a live pull request companion for Discord, powered by Rust.

[![Rust](https://img.shields.io/badge/rust-1.77%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Discord](https://img.shields.io/badge/discord-slash--commands-5865F2.svg)](https://discord.com)

DiGiBot listens to GitHub webhook events and maintains a single, always-up-to-date Discord message for each open pull request. Reviewer status, commit activity, and PR state stay in sync — no channel clutter, no manual refreshing.

> **Status:** Active development — see [Roadmap](#roadmap) for what's landed and what's next.

---

## Features

- **One message per PR** — title, latest commit, status emoji, and reviewer list update in place as events arrive.
- **Audit thread** — every event is appended to a private thread under the main message for full traceability.
- **Reviewer assignment** — assign reviewers via slash command or button; GitHub ↔ Discord username linking means `@mentions` just work.
- **Smart reminders** — ping reviewers by DM through the bot, with a 3-hour cooldown that resets on new activity.
- **Push-aware** — `push` events to `main` (including merges) are reflected in the PR's commit line.
- **Slash commands** — `/subscribe`, `/link`, `/assign`, and more — all ephemeral, no channel spam.
- **Lightweight persistence** — SQLite-backed subscriptions and user links survive restarts; everything else lives in memory.
- **Health endpoint** — `GET /health` reports bot and GitHub API reachability.

---

## How It Works

```
GitHub webhook (pull_request, pull_request_review, push)
│
▼
┌──────────────────────┐
│   Axum HTTP server   │  ← verifies HMAC-SHA256 signature
│   /github/webhook    │
└──────────┬───────────┘
           │  deserialized event
           ▼
┌──────────────────────┐
│    Event handler     │  ← translates event → message update
└──────────┬───────────┘
           │
           ▼
┌──────────────────────┐
│  Discord (Serenity)  │  ← edits main message, appends to audit thread
└──────────────────────┘
```

1. A pull request is opened, reviewed, or merged on GitHub.
2. GitHub sends a signed webhook payload to DiGiBot.
3. DiGiBot updates the corresponding pinned message in the subscribed Discord channel and appends an entry to the audit thread.
4. Users interact through slash commands and buttons — everything stays in one place.

---

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.9+
- A [Discord application](https://discord.com/developers/applications) with a bot token
- A [GitHub personal access token](https://github.com/settings/tokens) with `repo` scope
- A webhook secret (any cryptographically random string, e.g. `openssl rand -hex 32`)

### Build & Run

```bash
git clone https://github.com/kevinlizarazu/digibot.git
cd digibot

cargo build --release

DISCORD_TOKEN=...              \
GITHUB_WEBHOOK_SECRET=...      \
GITHUB_REPO=owner/repo         \
GITHUB_TOKEN=...               \
RUST_LOG=info                  \
./target/release/digibot
```

### Deploy on Railway

1. Push the repository to GitHub.
2. In Railway, create a new project from the repo.
3. Set the environment variables below in the service settings.
4. Expose `$PORT` and point your GitHub webhook to `https://<your-domain>/github/webhook`.

### Environment Variables

| Variable                 | Required | Description                                         |
|--------------------------|----------|-----------------------------------------------------|
| `DISCORD_TOKEN`          | Yes      | Discord bot token                                   |
| `GITHUB_WEBHOOK_SECRET`  | Yes      | Secret used to verify HMAC-SHA256 webhook payloads  |
| `GITHUB_REPO`            | Yes      | Repository to watch (`owner/name`)                  |
| `GITHUB_TOKEN`           | Yes      | GitHub PAT for API calls (reviewer assignment, etc.)|
| `RUST_LOG`               | No       | Log level: `trace`, `debug`, `info`, `warn`         |
| `PORT`                   | No       | HTTP listen port (default: `3000`)                  |

### Invite the Bot

Generate an OAuth2 invite URL with the following scopes and permissions:

**Scopes:** `bot`, `applications.commands`

**Bot permissions:**
- Send Messages
- Create Public Threads
- Send Messages in Threads
- Manage Messages
- Use Slash Commands

---

## Slash Commands

| Command        | Description                                              |
|----------------|----------------------------------------------------------|
| `/subscribe`   | Subscribe the current channel to PR updates              |
| `/unsubscribe` | Remove the channel subscription                          |
| `/link`        | Link your Discord account to a GitHub username           |
| `/unlink`      | Remove your Discord ↔ GitHub link                        |
| `/assign`      | Assign a reviewer to a PR (autocomplete for PR and user) |
| `/healthz`     | Show bot and GitHub API status                           |

---

## Status Emojis

| Emoji | Meaning         |
|-------|-----------------|
| 🟢    | Open            |
| 💬    | Review activity |
| 🟣    | Merged          |
| 🔴    | Closed          |
| ❌    | Deleted         |

---

## Roadmap

### v0.1 — Core loop ✅
- Webhook verification and event deserialization
- Main message creation and in-place updates (PR events)
- Audit thread logging
- Slash commands: `/subscribe`, `/unsubscribe`, `/link`, `/healthz`
- SQLite persistence for subscriptions and user links

### v0.2 — Review lifecycle
- Reviewer assignment via `/assign` slash command
- Reviewer approval status display (✅ / ❌)
- Remind button with DM delivery and 3-hour cooldown
- Push-to-main reflected in commit line

### v0.3 — Polish
- Structured logging (JSON-compatible for log aggregators)
- Multi-repo support
- GitHub profile auto-discovery for Discord tag resolution
- Web dashboard for configuration

---

### Key Dependencies

| Crate       | Purpose                              |
|-------------|--------------------------------------|
| `axum`      | HTTP server + webhook endpoint       |
| `serenity`  | Discord gateway + slash commands     |
| `octocrab`  | GitHub REST API client               |
| `sqlx`      | SQLite persistence                   |
| `tokio`     | Async runtime                        |
| `tracing`   | Structured logging                   |

### Design Principles

DiGiBot's module boundaries follow SOLID:

- **Single responsibility** — each module does one thing: webhooks, Discord messaging, GitHub API calls, state persistence.
- **Open/closed** — state is abstracted behind traits; swap SQLite for PostgreSQL without touching event handlers.
- **Interface segregation** — separate focused traits for subscriptions, user links, and cooldowns rather than one monolithic store interface.
- **Dependency inversion** — high-level handlers depend on trait interfaces, not concrete database types.

---

## Development

```bash
# Run with hot-reload (requires cargo-watch)
cargo watch -x run

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Check formatting
cargo fmt -- --check
```

> **Tip:** Set `RUST_LOG=debug` locally to see full event payloads during webhook development.

---

## Contributing

Issues and PRs are welcome. Please run `cargo fmt` and `cargo clippy` before opening a pull request.

---

## License

MIT © Kevin Lizarazu