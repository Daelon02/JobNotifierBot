use super::db::PostgresDb;
use super::models::{ApplicationDb, BlacklistDb, Keyword, NewVacancy, UserDb, VacancyDb};
use crate::error::AppResult;

#[derive(Clone)]
pub struct Db {
    postgres: PostgresDb,
}

impl Db {
    pub async fn new(database_url: &str) -> AppResult<Self> {
        let postgres = PostgresDb::new(database_url).await?;
        Ok(Self { postgres })
    }

    pub async fn get_or_create_user(&self, telegram_id: i64) -> AppResult<UserDb> {
        self.postgres.get_or_create_user(telegram_id).await
    }

    pub async fn get_user(&self, telegram_id: i64) -> AppResult<Option<UserDb>> {
        self.postgres.get_user(telegram_id).await
    }

    pub async fn get_all_users(&self) -> AppResult<Vec<UserDb>> {
        self.postgres.get_all_users().await
    }

    pub async fn toggle_keyword(
        &self,
        telegram_id: i64,
        keyword: Keyword,
        enable: bool,
    ) -> AppResult<()> {
        self.postgres
            .toggle_keyword(telegram_id, keyword, enable)
            .await
    }

    pub async fn update_user_cv(
        &self,
        telegram_id: i64,
        file_path: &str,
        file_name: &str,
    ) -> AppResult<()> {
        self.postgres
            .update_user_cv(telegram_id, file_path, file_name)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_user_email(
        &self,
        telegram_id: i64,
        email: &str,
        imap_host: &str,
        imap_port: i32,
        imap_user: &str,
        imap_pass: &str,
        smtp_host: &str,
        smtp_port: i32,
    ) -> AppResult<()> {
        self.postgres
            .update_user_email(
                telegram_id,
                email,
                imap_host,
                imap_port,
                imap_user,
                imap_pass,
                smtp_host,
                smtp_port,
            )
            .await
    }

    pub async fn save_vacancy(&self, vacancy: NewVacancy<'_>) -> AppResult<bool> {
        self.postgres.save_vacancy(vacancy).await
    }

    pub async fn get_vacancy(&self, vacancy_id: &str) -> AppResult<Option<VacancyDb>> {
        self.postgres.get_vacancy(vacancy_id).await
    }

    pub async fn get_recent_vacancies(
        &self,
        keyword: Option<Keyword>,
        limit: i64,
    ) -> AppResult<Vec<VacancyDb>> {
        self.postgres.get_recent_vacancies(keyword, limit).await
    }

    pub async fn add_application(
        &self,
        user_id: i64,
        vacancy_id: &str,
        status: &str,
    ) -> AppResult<()> {
        self.postgres
            .add_application(user_id, vacancy_id, status)
            .await
    }

    pub async fn get_user_applications(
        &self,
        user_id: i64,
    ) -> AppResult<Vec<(ApplicationDb, VacancyDb)>> {
        self.postgres.get_user_applications(user_id).await
    }

    pub async fn add_blacklist(&self, user_id: i64, company_name: &str) -> AppResult<()> {
        self.postgres.add_blacklist(user_id, company_name).await
    }

    pub async fn remove_blacklist(&self, user_id: i64, company_name: &str) -> AppResult<()> {
        self.postgres.remove_blacklist(user_id, company_name).await
    }

    pub async fn get_user_blacklist(&self, user_id: i64) -> AppResult<Vec<BlacklistDb>> {
        self.postgres.get_user_blacklist(user_id).await
    }

    pub async fn is_company_blacklisted(
        &self,
        user_id: i64,
        company_name: &str,
    ) -> AppResult<bool> {
        self.postgres
            .is_company_blacklisted(user_id, company_name)
            .await
    }
}
