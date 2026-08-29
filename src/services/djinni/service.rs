use super::models::DjinniVacancy;
use crate::error::{AppError, AppResult};
use crate::services::storage::models::{Keyword, NewVacancy, Platform};
use crate::services::storage::service::Db;
use log::{error, info, warn};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

const DJINNI_BASE_URL: &str = "https://djinni.co";
const DJINNI_JOBS_URL: &str = "https://djinni.co/jobs/";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct DjinniClient {
    client: reqwest::Client,
}

impl DjinniClient {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    pub async fn fetch_vacancies(&self, keyword: Keyword) -> AppResult<Vec<DjinniVacancy>> {
        let keyword_query = match keyword {
            Keyword::Rust => "Rust",
            Keyword::Go => "Golang",
        };

        let url = format!("{}?primary_keyword={}", DJINNI_JOBS_URL, keyword_query);
        let resp = self.client.get(&url).send().await?.text().await?;

        self.parse_html(&resp, keyword)
    }

    pub fn parse_html(
        &self,
        html_content: &str,
        keyword: Keyword,
    ) -> AppResult<Vec<DjinniVacancy>> {
        let document = Html::parse_document(html_content);

        let item_selector = Selector::parse(
            "div.job-item, div[id^='job-item-'], li.list-jobs__item, .job-list-item",
        )
        .map_err(|e| AppError::Scraper(format!("Invalid item selector: {:?}", e)))?;
        let title_selector = Selector::parse(
            ".job-item__position, .job-list-item__title a, .list-jobs__title a, a.profile, h2",
        )
        .map_err(|e| AppError::Scraper(format!("Invalid title selector: {:?}", e)))?;
        let link_selector =
            Selector::parse("a.job_item__header-link, .job-list-item__title a, .list-jobs__title a, a[href*='/jobs/']")
                .map_err(|e| AppError::Scraper(format!("Invalid link selector: {:?}", e)))?;
        let company_selector = Selector::parse(
            "span.text-gray-800, a[href*='/jobs/company-'], .list-jobs__details a, .job-list-item__company, .font-weight-500",
        )
        .map_err(|e| AppError::Scraper(format!("Invalid company selector: {:?}", e)))?;
        let salary_selector = Selector::parse(
            ".public-salary-item, .text-success, .public-salary, strong.text-success, header .col-auto span, .text-body-tertiary",
        )
        .map_err(|e| AppError::Scraper(format!("Invalid salary selector: {:?}", e)))?;
        let desc_selector =
            Selector::parse(".js-truncated-text, .job-list-item__description, .list-jobs__description, .text-card, div[id^='job-description-']")
                .map_err(|e| AppError::Scraper(format!("Invalid desc selector: {:?}", e)))?;
        let tag_selector =
            Selector::parse(".job-item__tags span, .job-item__tags a, .job-list-item__tags span, .job-list-item__job-info span, .location-text, .fw-medium span")
                .map_err(|e| AppError::Scraper(format!("Invalid tag selector: {:?}", e)))?;

        let mut vacancies = Vec::new();

        for element in document.select(&item_selector) {
            let relative_url = element
                .select(&link_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .unwrap_or_default();

            let title = if let Some(el) = element.select(&title_selector).next() {
                el.text().collect::<Vec<_>>().join(" ").trim().to_string()
            } else {
                continue;
            };

            let full_url = if relative_url.starts_with("http") {
                relative_url.to_string()
            } else if !relative_url.is_empty() {
                format!("{}{}", DJINNI_BASE_URL, relative_url)
            } else {
                continue;
            };

            let company = element
                .select(&company_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_else(|| "Unknown Company".to_string());

            let salary = element
                .select(&salary_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty());

            let summary = element
                .select(&desc_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_else(|| "No description provided".to_string());

            let stack_tags = element
                .select(&tag_selector)
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty() && s != "·")
                .collect::<Vec<_>>();

            let stack = if stack_tags.is_empty() {
                None
            } else {
                Some(stack_tags.join(", "))
            };

            // Strictly filter out vacancies that do not contain the target keyword
            if !keyword.is_match(&title, stack.as_deref(), &summary) {
                continue;
            }

            let mut hasher = Sha256::new();
            hasher.update(full_url.as_bytes());
            let hash_str = format!("{:x}", hasher.finalize());
            let id = format!("djinni_{}", &hash_str[..16]);

            if !title.is_empty() && !full_url.is_empty() {
                vacancies.push(DjinniVacancy {
                    id,
                    keyword,
                    title,
                    company,
                    salary,
                    stack,
                    summary: summary.chars().take(400).collect(),
                    url: full_url,
                });
            }
        }

        Ok(vacancies)
    }
}

impl Default for DjinniClient {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run_djinni_polling(
    client: DjinniClient,
    db: Db,
    bot: Bot,
    interval_seconds: u64,
) -> AppResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

    loop {
        interval.tick().await;
        info!("Running Djinni vacancy scraper...");

        for keyword in [Keyword::Rust, Keyword::Go] {
            match client.fetch_vacancies(keyword).await {
                Ok(vacancies) => {
                    info!(
                        "Djinni scraper fetched {} vacancies for {:?}",
                        vacancies.len(),
                        keyword
                    );

                    for vac in vacancies {
                        let new_vac = NewVacancy {
                            id: &vac.id,
                            platform: Platform::Djinni.as_str(),
                            keyword: vac.keyword.as_str(),
                            title: &vac.title,
                            company: &vac.company,
                            salary: vac.salary.as_deref(),
                            stack: vac.stack.as_deref(),
                            summary: &vac.summary,
                            url: &vac.url,
                        };

                        match db.save_vacancy(new_vac).await {
                            Ok(true) => {
                                info!("Discovered new Djinni vacancy: {}", vac.title);
                                notify_users_new_vacancy(&db, &bot, &vac).await;
                            }
                            Ok(false) => {
                                // Already known vacancy
                            }
                            Err(e) => {
                                error!("Failed to save Djinni vacancy {}: {:?}", vac.id, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Djinni scraper error for {:?}: {:?}", keyword, e);
                }
            }
        }
    }
}

async fn notify_users_new_vacancy(db: &Db, bot: &Bot, vac: &DjinniVacancy) {
    let users = match db.get_all_users().await {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to load users for notification: {:?}", e);
            return;
        }
    };

    for user in users {
        let is_keyword_enabled = match vac.keyword {
            Keyword::Rust => user.track_rust,
            Keyword::Go => user.track_go,
        };

        if !is_keyword_enabled {
            continue;
        }

        let _ = send_djinni_vacancy_to_user(db, bot, user.telegram_id, vac).await;
    }
}

pub async fn send_djinni_vacancy_to_user(
    db: &Db,
    bot: &Bot,
    user_id: i64,
    vac: &DjinniVacancy,
) -> AppResult<bool> {
    match db.is_company_blacklisted(user_id, &vac.company).await {
        Ok(true) => {
            info!(
                "Skipping Djinni notification for user {} (company '{}' is blacklisted)",
                user_id, vac.company
            );
            return Ok(false);
        }
        Ok(false) => {}
        Err(e) => {
            error!("Error checking blacklist for user {}: {:?}", user_id, e);
        }
    }

    let salary_line = vac
        .salary
        .as_ref()
        .map(|s| format!("💰 <b>Вилка:</b> {}\n", html_escape(s)))
        .unwrap_or_default();

    let stack_line = vac
        .stack
        .as_ref()
        .map(|st| format!("🛠 <b>Стек:</b> {}\n", html_escape(st)))
        .unwrap_or_default();

    let text = format!(
        "🔥 <b>Нова вакансія на Djinni!</b>\n\n\
         💼 <b>Позиція:</b> {}\n\
         🏢 <b>Компанія:</b> {}\n\
         🔑 <b>Тег:</b> #{}\n\
         {}{}\
         📝 <b>Опис:</b>\n<i>{}</i>\n\n\
         👇 <i>Оберіть дію:</i>",
        html_escape(&vac.title),
        html_escape(&vac.company),
        vac.keyword.display_name(),
        salary_line,
        stack_line,
        html_escape(&vac.summary)
    );

    let apply_data = format!("apply:{}", vac.id);
    let blacklist_data = format!("bl:{}", truncate_str(&vac.company, 40));

    let fallback_url = match reqwest::Url::parse("https://djinni.co") {
        Ok(u) => u,
        Err(_) => return Ok(false),
    };
    let open_url = reqwest::Url::parse(&vac.url).unwrap_or(fallback_url);

    let keyboard = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("📩 Надіслати резюме", apply_data),
            InlineKeyboardButton::callback("🚫 В блекліст", blacklist_data),
        ],
        vec![InlineKeyboardButton::url("🔗 Відкрити вакансію", open_url)],
    ]);

    bot.send_message(ChatId(user_id), text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(true)
}

pub fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn truncate_str(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
