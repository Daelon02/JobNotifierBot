use super::models::DouVacancy;
use crate::error::{AppError, AppResult};
use crate::services::djinni::service::{html_escape, truncate_str};
use crate::services::storage::models::{Keyword, NewVacancy, Platform};
use crate::services::storage::service::Db;
use log::{error, info, warn};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

const DOU_BASE_URL: &str = "https://jobs.dou.ua";
const DOU_JOBS_URL: &str = "https://jobs.dou.ua/vacancies/";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct DouClient {
    client: reqwest::Client,
}

impl DouClient {
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

    pub async fn fetch_vacancies(&self, keyword: Keyword) -> AppResult<Vec<DouVacancy>> {
        let keyword_query = match keyword {
            Keyword::Rust => "Rust",
            Keyword::Go => "Golang",
        };

        let url = format!("{}?category={}", DOU_JOBS_URL, keyword_query);
        let resp = self.client.get(&url).send().await?.text().await?;

        self.parse_html(&resp, keyword)
    }

    pub fn parse_html(&self, html_content: &str, keyword: Keyword) -> AppResult<Vec<DouVacancy>> {
        let document = Html::parse_document(html_content);

        let item_selector = Selector::parse("li.l-vacancy, .vacancy")
            .map_err(|e| AppError::Scraper(format!("Invalid item selector: {:?}", e)))?;
        let title_selector = Selector::parse("a.vt")
            .map_err(|e| AppError::Scraper(format!("Invalid title selector: {:?}", e)))?;
        let company_selector = Selector::parse("a.company")
            .map_err(|e| AppError::Scraper(format!("Invalid company selector: {:?}", e)))?;
        let salary_selector = Selector::parse("span.salary")
            .map_err(|e| AppError::Scraper(format!("Invalid salary selector: {:?}", e)))?;
        let desc_selector = Selector::parse("div.sh-desc")
            .map_err(|e| AppError::Scraper(format!("Invalid desc selector: {:?}", e)))?;
        let cities_selector = Selector::parse("span.cities")
            .map_err(|e| AppError::Scraper(format!("Invalid cities selector: {:?}", e)))?;

        let mut vacancies = Vec::new();

        for element in document.select(&item_selector) {
            let title_elem = match element.select(&title_selector).next() {
                Some(el) => el,
                None => continue,
            };

            let title = title_elem
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            let relative_url = title_elem.value().attr("href").unwrap_or_default();
            let full_url = if relative_url.starts_with("http") {
                relative_url.to_string()
            } else {
                format!("{}{}", DOU_BASE_URL, relative_url)
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

            let cities = element
                .select(&cities_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty());

            // Strictly filter out vacancies that do not contain the target keyword
            if !keyword.is_match(&title, cities.as_deref(), &summary) {
                continue;
            }

            let mut hasher = Sha256::new();
            hasher.update(full_url.as_bytes());
            let hash_str = format!("{:x}", hasher.finalize());
            let id = format!("dou_{}", &hash_str[..16]);

            if !title.is_empty() && !full_url.is_empty() {
                vacancies.push(DouVacancy {
                    id,
                    keyword,
                    title,
                    company,
                    salary,
                    stack: cities,
                    summary: summary.chars().take(400).collect(),
                    url: full_url,
                });
            }
        }

        Ok(vacancies)
    }
}

impl Default for DouClient {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run_dou_polling(
    client: DouClient,
    db: Db,
    bot: Bot,
    interval_seconds: u64,
) -> AppResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

    loop {
        interval.tick().await;
        info!("Running DOU vacancy scraper...");

        for keyword in [Keyword::Rust, Keyword::Go] {
            match client.fetch_vacancies(keyword).await {
                Ok(vacancies) => {
                    info!(
                        "DOU scraper fetched {} vacancies for {:?}",
                        vacancies.len(),
                        keyword
                    );

                    for vac in vacancies {
                        let new_vac = NewVacancy {
                            id: &vac.id,
                            platform: Platform::Dou.as_str(),
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
                                info!("Discovered new DOU vacancy: {}", vac.title);
                                notify_users_new_dou_vacancy(&db, &bot, &vac).await;
                            }
                            Ok(false) => {
                                // Already known
                            }
                            Err(e) => {
                                error!("Failed to save DOU vacancy {}: {:?}", vac.id, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("DOU scraper error for {:?}: {:?}", keyword, e);
                }
            }
        }
    }
}

async fn notify_users_new_dou_vacancy(db: &Db, bot: &Bot, vac: &DouVacancy) {
    let users = match db.get_all_users().await {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to load users for DOU notification: {:?}", e);
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

        let _ = send_dou_vacancy_to_user(db, bot, user.telegram_id, vac).await;
    }
}

pub async fn send_dou_vacancy_to_user(
    db: &Db,
    bot: &Bot,
    user_id: i64,
    vac: &DouVacancy,
) -> AppResult<bool> {
    match db.is_company_blacklisted(user_id, &vac.company).await {
        Ok(true) => {
            info!(
                "Skipping DOU vacancy for user {} (company '{}' is blacklisted)",
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

    let location_line = vac
        .stack
        .as_ref()
        .map(|l| format!("📍 <b>Локація:</b> {}\n", html_escape(l)))
        .unwrap_or_default();

    let text = format!(
        "⚡️ <b>Нова вакансія на DOU!</b>\n\n\
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
        location_line,
        html_escape(&vac.summary)
    );

    let apply_data = format!("apply:{}", vac.id);
    let blacklist_data = format!("bl:{}", truncate_str(&vac.company, 40));

    let fallback_url = match reqwest::Url::parse("https://jobs.dou.ua") {
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
