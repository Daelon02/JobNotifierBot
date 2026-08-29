use crate::schema::{applications, blacklist, users, vacancies};
use chrono::{DateTime, Utc};
use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::Pg;
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::VarChar;
use diesel::{Insertable, Queryable, Selectable};
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, AsExpression, FromSqlRow,
)]
#[diesel(sql_type = VarChar)]
pub enum Platform {
    #[serde(rename = "djinni")]
    Djinni,
    #[serde(rename = "dou")]
    Dou,
    #[serde(rename = "linkedin")]
    LinkedIn,
}

impl Platform {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Platform::Djinni => "djinni",
            Platform::Dou => "dou",
            Platform::LinkedIn => "linkedin",
        }
    }
}

impl ToSql<VarChar, Pg> for Platform {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        out.write_all(self.as_str().as_bytes())?;
        Ok(serialize::IsNull::No)
    }
}

impl FromSql<VarChar, Pg> for Platform {
    fn from_sql(bytes: diesel::pg::PgValue<'_>) -> deserialize::Result<Self> {
        let s = <String as FromSql<VarChar, Pg>>::from_sql(bytes)?;
        std::str::FromStr::from_str(&s).map_err(Box::from)
    }
}

impl std::str::FromStr for Platform {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "djinni" => Ok(Platform::Djinni),
            "dou" => Ok(Platform::Dou),
            "linkedin" => Ok(Platform::LinkedIn),
            other => Err(format!("Unknown platform: {}", other)),
        }
    }
}

#[derive(
    Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, AsExpression, FromSqlRow,
)]
#[diesel(sql_type = VarChar)]
pub enum Keyword {
    #[serde(rename = "rust")]
    Rust,
    #[serde(rename = "go")]
    Go,
}

impl Keyword {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Keyword::Rust => "rust",
            Keyword::Go => "go",
        }
    }

    pub const fn display_name(&self) -> &'static str {
        match self {
            Keyword::Rust => "Rust",
            Keyword::Go => "Go",
        }
    }

    pub fn is_match(&self, title: &str, stack: Option<&str>, summary: &str) -> bool {
        let is_target_token = |text: &str| -> bool {
            text.split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| {
                    let w = word.to_lowercase();
                    match self {
                        Keyword::Rust => w == "rust" || w == "rustlang",
                        Keyword::Go => w == "golang" || w == "go",
                    }
                })
        };

        // 1. If keyword appears as a distinct word/token in the title, it's a direct match
        if is_target_token(title) {
            return true;
        }

        // 2. If keyword appears as a distinct word/token in the tech stack/tags, it's a match
        if stack.is_some_and(is_target_token) {
            return true;
        }

        // 3. In the summary/description:
        let summary_lower = summary.to_lowercase();
        match self {
            Keyword::Rust => is_target_token(&summary_lower),
            Keyword::Go => {
                if summary_lower.contains("golang")
                    || summary_lower.contains("goroutine")
                    || summary_lower.contains("gin-gonic")
                    || summary_lower.contains("gorm")
                {
                    return true;
                }

                let words: Vec<&str> = summary_lower
                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                    .filter(|w| !w.is_empty())
                    .collect();

                for (i, &w) in words.iter().enumerate() {
                    if w == "go" {
                        if i > 0 {
                            let prev = words[i - 1];
                            if prev == "senior"
                                || prev == "middle"
                                || prev == "junior"
                                || prev == "lead"
                                || prev == "backend"
                                || prev == "fullstack"
                                || prev == "write"
                                || prev == "writing"
                                || prev == "with"
                                || prev == "in"
                            {
                                return true;
                            }
                        }
                        if i + 1 < words.len() {
                            let next = words[i + 1];
                            if next == "developer"
                                || next == "engineer"
                                || next == "backend"
                                || next == "software"
                                || next == "programmer"
                                || next == "microservices"
                                || next == "development"
                                || next == "programming"
                                || next == "services"
                                || next == "code"
                                || next == "server"
                            {
                                return true;
                            }
                        }
                    }
                }

                false
            }
        }
    }
}

impl std::str::FromStr for Keyword {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rust" => Ok(Keyword::Rust),
            "go" | "golang" => Ok(Keyword::Go),
            other => Err(format!("Unknown keyword: {}", other)),
        }
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserDb {
    pub telegram_id: i64,
    pub track_rust: bool,
    pub track_go: bool,
    pub cv_file_path: Option<String>,
    pub cv_file_name: Option<String>,
    pub email_address: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<i32>,
    pub imap_user: Option<String>,
    pub imap_password: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub telegram_id: i64,
    pub track_rust: bool,
    pub track_go: bool,
    pub cv_file_path: Option<&'a str>,
    pub cv_file_name: Option<&'a str>,
    pub email_address: Option<&'a str>,
    pub imap_host: Option<&'a str>,
    pub imap_port: Option<i32>,
    pub imap_user: Option<&'a str>,
    pub imap_password: Option<&'a str>,
    pub smtp_host: Option<&'a str>,
    pub smtp_port: Option<i32>,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = vacancies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct VacancyDb {
    pub id: String,
    pub platform: String,
    pub keyword: String,
    pub title: String,
    pub company: String,
    pub salary: Option<String>,
    pub stack: Option<String>,
    pub summary: String,
    pub url: String,
    pub discovered_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = vacancies)]
pub struct NewVacancy<'a> {
    pub id: &'a str,
    pub platform: &'a str,
    pub keyword: &'a str,
    pub title: &'a str,
    pub company: &'a str,
    pub salary: Option<&'a str>,
    pub stack: Option<&'a str>,
    pub summary: &'a str,
    pub url: &'a str,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = applications)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ApplicationDb {
    pub id: i32,
    pub user_id: i64,
    pub vacancy_id: String,
    pub status: String,
    pub applied_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = applications)]
pub struct NewApplication<'a> {
    pub user_id: i64,
    pub vacancy_id: &'a str,
    pub status: &'a str,
}

#[derive(Debug, Clone, Queryable, Selectable, Serialize, Deserialize)]
#[diesel(table_name = blacklist)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct BlacklistDb {
    pub id: i32,
    pub user_id: i64,
    pub company_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = blacklist)]
pub struct NewBlacklist<'a> {
    pub user_id: i64,
    pub company_name: &'a str,
}
