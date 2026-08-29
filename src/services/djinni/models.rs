use crate::services::storage::models::Keyword;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DjinniVacancy {
    pub id: String,
    pub keyword: Keyword,
    pub title: String,
    pub company: String,
    pub salary: Option<String>,
    pub stack: Option<String>,
    pub summary: String,
    pub url: String,
}
