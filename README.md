# Telegram Job Notifier Bot

A high-performance, asynchronous Telegram Bot written in Rust that monitors vacancy postings on **Djinni**, **DOU**, and **LinkedIn** for **Rust** and **Go** keywords, provides one-click resume applications, tracks applied jobs, manages employer blacklists, and monitors incoming recruiter emails/offers with direct in-bot replies.

---

## Features

* **Multi-Platform Scraping**:
  - Periodically (every 10 minutes) parses public job listings from **Djinni**, **DOU**, and **LinkedIn**.
  - Extracts position title, company name, salary range (if listed), tech stack tags, location, and concise summaries.
* **Keyword Toggles (Rust & Go)**:
  - Configure default tracking in `config.yaml`.
  - Dynamically toggle notifications for **Rust** and/or **Go** per-user directly via the Telegram interface (**`⚙️ Фільтри мов`**).
* **Interactive Vacancy Cards & One-Click Apply**:
  - Automatically notifies registered users with formatted Markdown/HTML cards.
  - **`📩 Надіслати резюме`**: Records the application with status `Applied` (`Надіслано`) and provides the candidate's saved CV.
  - **`🚫 В блекліст`**: Adds the company to the user's blacklist with a single click.
  - **`🔗 Відкрити вакансію`**: Direct link to the vacancy post.
* **Resume (CV) Management**:
  - Upload or update your resume (PDF/DOCX) via the bot (**`📄 Моє резюме`** / `/cv`).
  - Automatically removes outdated files from local storage (`storage/resumes/`) upon replacement.
* **Application Tracking**:
  - Track submitted applications with statuses (`Applied`, `Viewed`, `Replied`, `Offer`, `Rejected`) via `/applied` or **`📋 Мої відгуки`**.
* **Employer Blacklist**:
  - Blacklisted companies will not trigger new vacancy notifications.
  - Direct messages or recruiter communications remain fully accessible.
* **Email & Job Offer Monitoring (IMAP / SMTP)**:
  - Periodically checks the user's mailbox (IMAP over TLS) for recruiter outreach and job offers.
  - Quick setup format: `email, imap_host, imap_port, imap_user, password, smtp_host, smtp_port`
  - Uses secure **App Passwords** (Google / Ukr.net / Microsoft) instead of main account passwords.
  - Rich notification preview with direct action buttons:
    - **`✍️ Відповісти через бота`**: Send an email response directly from Telegram via SMTP.
    - **`📧 Відкрити пошту`**: Direct webmail redirection.
* **Saved Vacancies Explorer (`📁 Збережені вакансії` / `/vacancies`)**:
  - View discovered vacancies stored in PostgreSQL with instant platform badges (`[Djinni]`, `[DOU]`, `[LinkedIn]`), salary ranges, and direct links.
  - Interactive inline filter buttons: `🔥 Всі`, `🦀 Тільки Rust`, `🐹 Тільки Go`.
* **Database & Persistence**:
  - Built with **PostgreSQL** and **Diesel** (`diesel-async` + connection pooling).
  - Embedded migrations (`diesel_migrations`) execute automatically on startup.
* **Session Storage (Redis)**:
  - Persistent Telegram dialogue states (`teloxide::RedisStorage` + Bincode) survive bot restarts without state loss.
* **Logging & Observability**:
  - Structured logging with `env_logger` and `log`.

---

## Project Structure

```
job_notifier_bot/
├── Cargo.toml
├── Dockerfile
├── docker-compose.yaml
├── config.yaml
├── example.yaml
├── migrations/
│   └── 2026-08-29-000000_create_tables/
│       ├── up.sql
│       └── down.sql
├── tests/
│   └── scraper_tests.rs
├── storage/
│   └── resumes/
└── src/
    ├── lib.rs
    ├── main.rs
    ├── config.rs
    ├── error.rs
    ├── schema.rs
    └── services/
        ├── mod.rs
        ├── storage/
        │   ├── mod.rs
        │   ├── models.rs
        │   ├── db.rs
        │   └── service.rs
        ├── telegram/
        │   ├── mod.rs
        │   ├── models.rs
        │   └── service.rs
        ├── djinni/
        │   ├── mod.rs
        │   ├── models.rs
        │   └── service.rs
        ├── dou/
        │   ├── mod.rs
        │   ├── models.rs
        │   └── service.rs
        ├── linkedin/
        │   ├── mod.rs
        │   ├── models.rs
        │   └── service.rs
        └── email/
            ├── mod.rs
            ├── models.rs
            └── service.rs
```

---

## Configuration (`config.yaml`)

Create a `config.yaml` file in the root directory (based on `example.yaml`):

```yaml
telegram:
  bot_token: "YOUR_TELEGRAM_BOT_TOKEN"

postgres:
  url: "postgres://job_notifier:password@127.0.0.1:5432/job_notifier"

redis:
  url: "redis://127.0.0.1:6379"

keywords:
  track_rust: false
  track_go: false

scraping:
  interval_seconds: 600
```

---

## Quick Start (Local Development)

### 1. Start PostgreSQL
```bash
docker-compose up -d postgres
```

### 2. Run Database Migrations & Bot
```bash
cargo run --release config.yaml
```

---

## Deployment with Docker Compose

### 1. Prepare Configuration
Ensure your `config.yaml` has your Telegram Bot Token and valid parameters.

### 2. Start Full Stack
```bash
docker compose pull
docker compose up -d
```
The stack will pull `daelon02/job_notifier_bot:latest`, start Redis, initialize PostgreSQL, run migrations, and launch all background scrapers.

### 3. View Logs
```bash
docker compose logs -f bot
```

---

## Bot Commands & Usage

| Command | Description |
| :--- | :--- |
| `/start` | Open the main menu, view current setup status and toggle settings. |
| `/cv` | Upload, view, or replace your saved CV document. |
| `/filters` | Toggle keyword notifications (**Rust** / **Go**) on or off. |
| `/applied` | View the list of all applied jobs and their current statuses. |
| `/blacklist` | Manage blacklisted companies (view list, unblock, or add manually). |
| `/email` | Configure IMAP/SMTP credentials for job offer monitoring. |
| `/cancel` | Cancel any active interactive prompt. |
| `/help` | Detailed instructions and feature breakdown. |

---

## Testing & Linting

Run test suite and linters:
```bash
cargo fmt --all
cargo build --release
cargo clippy --workspace --tests -- -D warnings
cargo test --release
```
