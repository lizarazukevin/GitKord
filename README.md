[![Rust](https://img.shields.io/badge/rust-1.91%2B-orange.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/lizarazukevin/GitKord/actions/workflows/ci.yml/badge.svg)](https://github.com/lizarazukevin/GitKord/actions/workflows/ci.yml)
[![GitHub App](https://img.shields.io/badge/GitHub-App-181717?logo=github)](https://github.com/apps/gitkord)
[![Invite Bot](https://img.shields.io/badge/Discord-Invite-5865F2?logo=discord)](https://discord.com/oauth2/authorize?client_id=1503129643467673762)
[![Railway](https://img.shields.io/badge/Hosted%20on-Railway-0B0D0E?logo=railway&logoColor=white)](https://railway.app)

<h1 align="center">
  <img src="https://avatars.githubusercontent.com/in/3948389" width="128" height="128" style="border-radius: 50%;" alt="GitKord Logo">
  <br>GitKord
</h1>

<p align="center">
    A live pull request companion for Discord, powered by Rust.
    <br />
    One message per pull request, edited in place, with a per-PR audit thread.
    <br />
    <a href="#about">About</a>
    ·
    <a href="#quickstart">Quickstart</a>
    ·
    <a href="#features">Features</a>
    ·
    <a href="#contributing-and-development">Contributing</a>
    ·
    <a href="#roadmap">Roadmap</a>
  </p>

---

## About

Reviews slip through the cracks when the pull request lives on GitHub and the conversation lives in Discord. You either tab back and forth all day or you paste links into a channel and watch them go stale within the hour.

GitKord closes that gap. Point it at a repository and it keeps one message per pull request in your Discord channels, editing that message in place as the PR moves. Open it, review it, push to it, merge it, and the message follows along. Every event lands in a thread hanging off that message, so the full history of a PR reads top to bottom without anyone lifting a finger.

It runs as a GitHub App, so there is nothing to wire up per repository once it is installed. Subscribe a channel with a slash command and you are done!

> **Status:** Active development. See the [Roadmap](#roadmap) for what has landed and what is next. Join the [support channel](https://discord.gg/gNEvCUFzt) to keep up with updates.

## Quickstart

Getting GitKord watching a repository takes two installs and one command.

**1. Install the GitHub App** – Add [GitKord](https://github.com/apps/gitkord) to the repositories you want to track. This grants read access to pull requests and lets GitHub deliver events to the bot. No webhook setup on your end.

**2. Invite the bot to your server** – Use the [invite link](https://discord.com/oauth2/authorize?client_id=1503129643467673762).

**3. Subscribe a channel** – In any channel you want updates in, run:

```
/subscribe repository: owner/name
```

That is it. The next time someone opens a pull request on that repository, it shows up in the channel and stays current on its own.

| Action | Link |
|--------|------|
| Install the GitHub App | [github.com/apps/gitkord](https://github.com/apps/gitkord) |
| Invite the Discord bot | [Add to server](https://discord.com/oauth2/authorize?client_id=1503129643467673762) |

## Features

**One message per pull request.** Title, status, author, and branch update in place as events arrive. No stream of duplicate notifications, no scrolling to find the latest state.

**An audit thread for every PR.** Opened, reviewed, assigned, pushed to, closed: each event appends to a thread on the main message, so the story of a PR reads in order.

**The same repo in as many channels as you want.** Subscribe a repository across several channels in a server and each one gets its own message and thread. Teams that split by squad or by topic do not have to share a firehose.

**Reviewer assignment from Discord.** `/assign` and `/unassign` take a GitHub username or a Discord mention. Run them inside a PR thread and GitKord infers the repository and PR number for you. It will not let someone request a review from themselves.

**Linked identities.** `/link` ties your Discord account to your GitHub username after checking the account exists. Once linked, you show up as a Discord mention in the automated messages instead of a bare login.

**Quiet by design.** Every slash command replies privately to the person who ran it, so the channel stays free of command chatter.

**Health at a glance.** A `/health` command reports whether the bot and its GitHub connection are up, and an HTTP health endpoint is there for your uptime monitor.

### Slash commands

| Command | What it does |
|---------|--------------|
| `/subscribe` | Start posting PR updates for a repository in this channel. |
| `/unsubscribe` | Stop posting PR updates for a repository in this channel. |
| `/link` | Link your Discord account to a GitHub username. |
| `/unlink` | Remove your Discord to GitHub link. |
| `/assign` | Request a review. Skip `repo` and `pr` when run inside a PR thread. |
| `/unassign` | Remove a review request. Same thread awareness as `/assign`. |
| `/health` | Check that GitKord and its GitHub connection are online. |

### Reading the messages

Pull request status:

| Emoji | Meaning |
|-------|---------|
| 🟢 | Open |
| 🔴 | Closed |
| 🟣 | Merged |

Review verdicts:

| Emoji | Meaning           |
|-------|-------------------|
| ✅ | Approved          |
| 🛑 | Changes requested |
| 💬 | Commented         |
| 🟡 | Pending           |

## Contributing and development

Contributions are welcome. Before opening a pull request, run the format and lint checks below and make sure the test suite passes.

```bash
cargo fmt -- --check          # formatting
cargo clippy -- -D warnings   # lints, warnings treated as errors
cargo test                    # tests
```

`cargo clippy` runs against the pedantic and nursery lint groups configured in the crate, so expect it to be opinionated.

### Running locally

You do not need the GitHub App to develop against GitKord. In local dev mode it registers a webhook directly on the repository using a personal access token and points it at your tunnel, so real GitHub events reach the bot on your machine.

You will need [Rust](https://www.rust-lang.org/tools/install) 1.91 or newer, [Docker](https://docs.docker.com/get-docker/) (to run Postgres) or a local [Postgres](https://www.postgresql.org/) installation, a personal [Discord bot application](https://discord.com/developers/applications), a [GitHub personal access token](https://github.com/settings/tokens) with `repo` scope, and [ngrok](https://ngrok.com) (or any tunnel) to expose your local port.

#### Creating a Discord bot application

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications) and click **New Application**. Give it a name and click **Create**.
2. Go to the **Bot** tab in the left sidebar. Click **Reset Token** and copy the token that appears — this is your `DISCORD_TOKEN`. Store it somewhere secure.
3. Under the **OAuth2 → URL Generator** tab, select the following:
   - **Scopes**: `bot`, `applications.commands`
   - **Bot Permissions**: `Send Messages`, `Manage Threads`, `Read Message History`, `Send Messages in Threads`, `Embed Links`
4. Use the generated URL to invite the bot to a test Discord server.

#### Testing Changes
Set `LOCAL_DEV=true` and provide the following:

| Variable | Required | Description |
|----------|----------|-------------|
| `DISCORD_TOKEN` | Yes | Discord bot token from the Developer Portal (Bot → Reset Token). |
| `GITHUB_WEBHOOK_SECRET` | Yes | HMAC secret for verifying webhook payloads (`openssl rand -hex 32`). |
| `GITHUB_TOKEN` | Yes | GitHub PAT with `repo` scope, used instead of GitHub App auth. |
| `PUBLIC_DOMAIN` | Yes | Your tunnel host, e.g. `abc123.ngrok-free.app` (no `https://`). |
| `DATABASE_URL` | Yes | Postgres connection string. |
| `LOCAL_DEV` | No | Set to `true` to enable local dev mode. Defaults to `false`. |
| `RUST_LOG` | No | Log level: `trace`, `debug`, `info`, `warn`. |
| `PORT` | No | HTTP listen port. Defaults to `3000`. |

A typical loop, in three terminals:

```bash
# 1. Postgres
docker run -d --name gitkord-pg \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=gitkord \
  -p 5432:5432 \
  postgres:16

# 2. Tunnel
ngrok http 3000

# 3. The bot
LOCAL_DEV=true \
DISCORD_TOKEN=... \
GITHUB_WEBHOOK_SECRET=... \
GITHUB_TOKEN=ghp_... \
PUBLIC_DOMAIN=your-tunnel-host.ngrok-free.app \
DATABASE_URL=postgres://postgres:password@localhost:5432/gitkord \
cargo run
```

Set `RUST_LOG=debug` to see full event payloads while you work. If you have `cargo-watch` installed, `cargo watch -x run` rebuilds on save.

## Roadmap

#### Shipped:

✅ Webhook ingestion with HMAC-SHA256 signature verification.<br>
✅ One live message per pull request, edited in place, with a per-PR audit thread.<br>
✅ `pull_request` and `pull_request_review` events reflected in Discord.<br>
✅ Subscriptions from any channel to any repository, including the same repo across multiple channels in a server.<br>
✅ GitHub App installation auth, so no per-repo webhook setup.<br>
✅ `/link` and `/unlink` with GitHub account verification.<br>
✅ `/assign` and `/unassign` with Discord mention resolution, thread-aware context, and a self-assignment guard.<br>
✅ Commit pushes to an open PR refresh its message everywhere it is posted.<br>
✅ Railway deployment on a persistent URL.<br>
✅ Error handling packaged in user-friendly ephemeral messages.<br>

#### In progress:

- [ ] A broader test suite covering signature verification, persistence, and formatting.

#### Planned:

- [ ] Checks handling for GitHub actions.
- [ ] AI summary view of new PRs.
- [ ] Dedicated GitKord domain.

## License

MIT, © Kevin Lizarazu. See [LICENSE](LICENSE).
