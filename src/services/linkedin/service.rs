use super::models::LinkedInVacancy;
use crate::error::{AppError, AppResult};
use crate::services::djinni::service::{html_escape, truncate_str};
use crate::services::storage::models::{Keyword, NewVacancy, Platform};
use crate::services::storage::service::Db;
use log::{error, info, warn};
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, HeaderMap, HeaderValue, USER_AGENT};
use scraper::{Html, Selector};
use sha2::{Digest, Sha256};
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup};

const LINKEDIN_JOBS_API: &str =
    "https://www.linkedin.com/jobs-guest/jobs/api/seeMoreJobPostings/search";
const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

#[derive(Clone)]
pub struct LinkedInClient {
    client: reqwest::Client,
}

impl LinkedInClient {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static(
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            ),
        );
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("en-US,en;q=0.9,uk;q=0.8"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    pub async fn fetch_vacancies(&self, keyword: Keyword) -> AppResult<Vec<LinkedInVacancy>> {
        let keyword_query = match keyword {
            Keyword::Rust => "Rust developer",
            Keyword::Go => "Golang developer",
        };

        let url = format!(
            "{}?keywords={}&location=Ukraine&f_TPR=r86400&position=1&pageNum=0",
            LINKEDIN_JOBS_API,
            urlencoding::encode(keyword_query)
        );

        let resp = self.client.get(&url).send().await?.text().await?;
        self.parse_html(&resp, keyword)
    }

    pub fn parse_html(
        &self,
        html_content: &str,
        keyword: Keyword,
    ) -> AppResult<Vec<LinkedInVacancy>> {
        let document = Html::parse_document(html_content);

        let item_selector = Selector::parse("li, .base-card, .job-search-card")
            .map_err(|e| AppError::Scraper(format!("Invalid item selector: {:?}", e)))?;
        let title_selector = Selector::parse(".base-search-card__title, h3")
            .map_err(|e| AppError::Scraper(format!("Invalid title selector: {:?}", e)))?;
        let link_selector = Selector::parse("a.base-card__full-link, a")
            .map_err(|e| AppError::Scraper(format!("Invalid link selector: {:?}", e)))?;
        let company_selector = Selector::parse(".base-search-card__subtitle, h4")
            .map_err(|e| AppError::Scraper(format!("Invalid company selector: {:?}", e)))?;
        let location_selector = Selector::parse(".job-search-card__location")
            .map_err(|e| AppError::Scraper(format!("Invalid location selector: {:?}", e)))?;

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

            let link_elem = match element.select(&link_selector).next() {
                Some(el) => el,
                None => continue,
            };

            let full_url = link_elem.value().attr("href").unwrap_or_default().trim();
            if full_url.is_empty() || title.is_empty() {
                continue;
            }

            let company = element
                .select(&company_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .unwrap_or_else(|| "Unknown Company".to_string());

            let location = element
                .select(&location_selector)
                .next()
                .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
                .filter(|s| !s.is_empty());

            // Strictly filter out vacancies that do not contain the target keyword
            if !keyword.is_match(&title, location.as_deref(), "") {
                continue;
            }

            // Clean up tracking params from URL
            let clean_url = full_url.split('?').next().unwrap_or(full_url).to_string();

            let mut hasher = Sha256::new();
            hasher.update(clean_url.as_bytes());
            let hash_str = format!("{:x}", hasher.finalize());
            let id = format!("linkedin_{}", &hash_str[..16]);

            vacancies.push(LinkedInVacancy {
                id,
                keyword,
                title,
                company,
                salary: None,
                stack: location,
                summary: "Перегляньте детальний опис позиції та вимоги на сторінці LinkedIn"
                    .to_string(),
                url: clean_url,
            });
        }

        Ok(vacancies)
    }
}

impl Default for LinkedInClient {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn run_linkedin_polling(
    client: LinkedInClient,
    db: Db,
    bot: Bot,
    interval_seconds: u64,
) -> AppResult<()> {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_seconds));

    loop {
        interval.tick().await;
        info!("Running LinkedIn vacancy scraper...");

        for keyword in [Keyword::Rust, Keyword::Go] {
            match client.fetch_vacancies(keyword).await {
                Ok(vacancies) => {
                    info!(
                        "LinkedIn scraper fetched {} vacancies for {:?}",
                        vacancies.len(),
                        keyword
                    );

                    for vac in vacancies {
                        let new_vac = NewVacancy {
                            id: &vac.id,
                            platform: Platform::LinkedIn.as_str(),
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
                                info!("Discovered new LinkedIn vacancy: {}", vac.title);
                                notify_users_new_linkedin_vacancy(&db, &bot, &vac).await;
                            }
                            Ok(false) => {
                                // Already known
                            }
                            Err(e) => {
                                error!("Failed to save LinkedIn vacancy {}: {:?}", vac.id, e);
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("LinkedIn scraper error for {:?}: {:?}", keyword, e);
                }
            }
        }
    }
}

async fn notify_users_new_linkedin_vacancy(db: &Db, bot: &Bot, vac: &LinkedInVacancy) {
    let users = match db.get_all_users().await {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to load users for LinkedIn notification: {:?}", e);
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

        let _ = send_linkedin_vacancy_to_user(db, bot, user.telegram_id, vac).await;
    }
}

pub async fn send_linkedin_vacancy_to_user(
    db: &Db,
    bot: &Bot,
    user_id: i64,
    vac: &LinkedInVacancy,
) -> AppResult<bool> {
    match db.is_company_blacklisted(user_id, &vac.company).await {
        Ok(true) => {
            info!(
                "Skipping LinkedIn vacancy for user {} (company '{}' is blacklisted)",
                user_id, vac.company
            );
            return Ok(false);
        }
        Ok(false) => {}
        Err(e) => {
            error!("Error checking blacklist for user {}: {:?}", user_id, e);
        }
    }

    let location_line = vac
        .stack
        .as_ref()
        .map(|l| format!("📍 <b>Локація:</b> {}\n", html_escape(l)))
        .unwrap_or_default();

    let text = format!(
        "🌐 <b>Нова вакансія на LinkedIn!</b>\n\n\
         💼 <b>Позиція:</b> {}\n\
         🏢 <b>Компанія:</b> {}\n\
         🔑 <b>Тег:</b> #{}\n\
         {}\
         📝 <b>Опис:</b>\n<i>{}</i>\n\n\
         👇 <i>Оберіть дію:</i>",
        html_escape(&vac.title),
        html_escape(&vac.company),
        vac.keyword.display_name(),
        location_line,
        html_escape(&vac.summary)
    );

    let apply_data = format!("apply:{}", vac.id);
    let blacklist_data = format!("bl:{}", truncate_str(&vac.company, 40));

    let fallback_url = match reqwest::Url::parse("https://www.linkedin.com") {
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
