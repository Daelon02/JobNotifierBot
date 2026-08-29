use super::models::JobEmailNotification;
use crate::error::{AppError, AppResult};
use crate::services::djinni::service::html_escape;
use crate::services::storage::models::UserDb;
use crate::services::storage::service::Db;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use log::{error, info, warn};
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

const EMAIL_KEYWORDS: &[&str] = &[
    "offer",
    "job",
    "interview",
    "вакансія",
    "офер",
    "співбесіда",
    "position",
    "djinni",
    "dou",
    "linkedin",
    "recruiter",
    "пропозиція",
    "запрошення",
];

pub struct EmailService;

impl EmailService {
    pub async fn send_reply(
        user: &UserDb,
        to_address: &str,
        subject: &str,
        body: &str,
    ) -> AppResult<()> {
        let smtp_host = user
            .smtp_host
            .as_deref()
            .ok_or_else(|| AppError::Email("SMTP host not configured".to_string()))?;
        let user_email = user
            .email_address
            .as_deref()
            .ok_or_else(|| AppError::Email("Email address not configured".to_string()))?;
        let imap_user = user.imap_user.as_deref().unwrap_or(user_email);
        let password = user
            .imap_password
            .as_deref()
            .ok_or_else(|| AppError::Email("Email password not configured".to_string()))?;

        let reply_subject = if subject.to_lowercase().starts_with("re:") {
            subject.to_string()
        } else {
            format!("Re: {}", subject)
        };

        let email = Message::builder()
            .from(user_email.parse()?)
            .to(to_address.parse()?)
            .subject(reply_subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())?;

        let creds = Credentials::new(imap_user.to_string(), password.to_string());

        let mailer: AsyncSmtpTransport<Tokio1Executor> =
            AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)?
                .credentials(creds)
                .build();

        mailer.send(email).await?;
        info!("Email reply sent successfully to {}", to_address);
        Ok(())
    }

    pub async fn check_inbox(user: &UserDb) -> AppResult<Vec<JobEmailNotification>> {
        let imap_host = match &user.imap_host {
            Some(h) => h.clone(),
            None => return Ok(Vec::new()),
        };
        let imap_port = user.imap_port.unwrap_or(993) as u16;
        let imap_user = match &user.imap_user {
            Some(u) => u.clone(),
            None => return Ok(Vec::new()),
        };
        let imap_password = match &user.imap_password {
            Some(p) => p.clone(),
            None => return Ok(Vec::new()),
        };

        let user_for_task = imap_user.clone();
        let pass_for_task = imap_password.clone();
        let host_for_task = imap_host.clone();

        let notifications =
            tokio::task::spawn_blocking(move || -> AppResult<Vec<JobEmailNotification>> {
                let tls = native_tls::TlsConnector::builder()
                    .build()
                    .map_err(|e| AppError::Email(format!("TLS error: {:?}", e)))?;

                let client =
                    imap::connect((host_for_task.as_str(), imap_port), &host_for_task, &tls)
                        .map_err(|e| {
                            AppError::Email(format!(
                                "Failed to connect to IMAP {}: {:?}",
                                host_for_task, e
                            ))
                        })?;

                let mut session = client
                    .login(&user_for_task, &pass_for_task)
                    .map_err(|e| AppError::Email(format!("Failed to login to IMAP: {:?}", e.0)))?;

                session
                    .select("INBOX")
                    .map_err(|e| AppError::Email(format!("Failed to select INBOX: {:?}", e)))?;

                let uids = session
                    .search("UNSEEN")
                    .map_err(|e| AppError::Email(format!("Failed to search UNSEEN: {:?}", e)))?;

                let mut uid_vec: Vec<u32> = uids.into_iter().collect();
                uid_vec.sort_unstable_by(|a, b| b.cmp(a));

                let mut results = Vec::new();

                for uid in uid_vec.iter().take(10) {
                    let messages = match session.fetch(uid.to_string(), "RFC822") {
                        Ok(m) => m,
                        Err(_) => continue,
                    };

                    for message in messages.iter() {
                        let body = match message.body() {
                            Some(b) => b,
                            None => continue,
                        };

                        if let Some(parsed) = mail_parser::MessageParser::default().parse(body) {
                            let subject = parsed.subject().unwrap_or("No Subject").to_string();
                            let from = parsed
                                .from()
                                .and_then(|f| f.first())
                                .map(|a| a.address().unwrap_or("unknown"))
                                .unwrap_or("unknown")
                                .to_string();

                            let text_body = parsed.body_text(0).unwrap_or_default();

                            let combined_text = format!("{} {}", subject, text_body).to_lowercase();
                            let is_job_related =
                                EMAIL_KEYWORDS.iter().any(|k| combined_text.contains(k));

                            if is_job_related {
                                let snippet: String = text_body.chars().take(300).collect();
                                let date_str = parsed
                                    .date()
                                    .map(|d| d.to_rfc3339())
                                    .unwrap_or_else(|| "Recent".to_string());

                                results.push(JobEmailNotification {
                                    message_id: uid.to_string(),
                                    from,
                                    subject,
                                    date: date_str,
                                    snippet,
                                });
                            }
                        }
                    }
                }

                let _ = session.logout();
                Ok(results)
            })
            .await
            .map_err(|e| AppError::Email(format!("Join error: {:?}", e)))??;

        Ok(notifications)
    }
}

pub async fn run_email_polling(db: Db, bot: Bot, interval_seconds: u64) -> AppResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

    loop {
        interval.tick().await;

        let users = match db.get_all_users().await {
            Ok(u) => u,
            Err(e) => {
                error!("Failed to fetch users for email polling: {:?}", e);
                continue;
            }
        };

        for user in users {
            if user.email_address.is_none() || user.imap_host.is_none() {
                continue;
            }

            match EmailService::check_inbox(&user).await {
                Ok(notifications) => {
                    for notif in notifications {
                        notify_user_email_offer(&bot, user.telegram_id, &notif).await;
                    }
                }
                Err(e) => {
                    warn!(
                        "Error checking email inbox for user {}: {:?}",
                        user.telegram_id, e
                    );
                }
            }
        }
    }
}

async fn notify_user_email_offer(bot: &Bot, telegram_id: i64, notif: &JobEmailNotification) {
    let text = format!(
        "📬 <b>Нове повідомлення по роботі / Офер на пошті!</b>\n\n\
         👤 <b>Від кого:</b> <code>{}</code>\n\
         📌 <b>Тема:</b> {}\n\
         📅 <b>Дата:</b> {}\n\n\
         📄 <b>Уривок:</b>\n<i>{}</i>\n\n\
         👇 <i>Ви можете відповісти через бота або відкрити пошту:</i>",
        html_escape(&notif.from),
        html_escape(&notif.subject),
        html_escape(&notif.date),
        html_escape(&notif.snippet)
    );

    let reply_cb = format!("reply_email:{}", notif.from);

    let webmail_url = if notif.from.contains("gmail") {
        "https://mail.google.com"
    } else {
        "https://webmail.com"
    };

    let fallback_url = match reqwest::Url::parse("https://mail.google.com") {
        Ok(u) => u,
        Err(_) => return,
    };
    let open_url = reqwest::Url::parse(webmail_url).unwrap_or(fallback_url);

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![InlineKeyboardButton::callback(
            "✍️ Відповісти через бота",
            reply_cb,
        )],
        vec![InlineKeyboardButton::url("📧 Відкрити пошту", open_url)],
    ]);

    if let Err(e) = bot
        .send_message(ChatId(telegram_id), text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await
    {
        error!(
            "Failed to send email job notification to {}: {:?}",
            telegram_id, e
        );
    }
}
