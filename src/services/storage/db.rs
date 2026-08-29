use super::models::{
    ApplicationDb, BlacklistDb, Keyword, NewApplication, NewBlacklist, NewUser, NewVacancy, UserDb,
    VacancyDb,
};
use crate::error::{AppError, AppResult};
use crate::schema::{applications, blacklist, users, vacancies};
use diesel::prelude::*;
use diesel_async::AsyncPgConnection;
use diesel_async::RunQueryDsl;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;

pub type PgPool = Pool<AsyncPgConnection>;

#[derive(Clone)]
pub struct PostgresDb {
    pool: PgPool,
}

impl PostgresDb {
    pub async fn new(database_url: &str) -> AppResult<Self> {
        let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
        let pool = Pool::builder()
            .build(config)
            .await
            .map_err(AppError::PoolError)?;

        Ok(Self { pool })
    }

    pub async fn get_or_create_user(&self, telegram_id: i64) -> AppResult<UserDb> {
        let mut conn = self.pool.get().await?;

        let existing: Option<UserDb> = users::table
            .filter(users::telegram_id.eq(telegram_id))
            .first(&mut conn)
            .await
            .optional()?;

        if let Some(user) = existing {
            return Ok(user);
        }

        let new_user = NewUser {
            telegram_id,
            track_rust: false,
            track_go: false,
            cv_file_path: None,
            cv_file_name: None,
            email_address: None,
            imap_host: None,
            imap_port: None,
            imap_user: None,
            imap_password: None,
            smtp_host: None,
            smtp_port: None,
        };

        let created: UserDb = diesel::insert_into(users::table)
            .values(&new_user)
            .on_conflict(users::telegram_id)
            .do_update()
            .set(users::telegram_id.eq(telegram_id))
            .get_result(&mut conn)
            .await?;

        Ok(created)
    }

    pub async fn get_user(&self, telegram_id: i64) -> AppResult<Option<UserDb>> {
        let mut conn = self.pool.get().await?;
        let user = users::table
            .filter(users::telegram_id.eq(telegram_id))
            .first(&mut conn)
            .await
            .optional()?;
        Ok(user)
    }

    pub async fn get_all_users(&self) -> AppResult<Vec<UserDb>> {
        let mut conn = self.pool.get().await?;
        let list = users::table.load::<UserDb>(&mut conn).await?;
        Ok(list)
    }

    pub async fn toggle_keyword(
        &self,
        telegram_id: i64,
        keyword: Keyword,
        enable: bool,
    ) -> AppResult<()> {
        let mut conn = self.pool.get().await?;
        match keyword {
            Keyword::Rust => {
                diesel::update(users::table.filter(users::telegram_id.eq(telegram_id)))
                    .set(users::track_rust.eq(enable))
                    .execute(&mut conn)
                    .await?;
            }
            Keyword::Go => {
                diesel::update(users::table.filter(users::telegram_id.eq(telegram_id)))
                    .set(users::track_go.eq(enable))
                    .execute(&mut conn)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn update_user_cv(
        &self,
        telegram_id: i64,
        file_path: &str,
        file_name: &str,
    ) -> AppResult<()> {
        let mut conn = self.pool.get().await?;
        diesel::update(users::table.filter(users::telegram_id.eq(telegram_id)))
            .set((
                users::cv_file_path.eq(Some(file_path)),
                users::cv_file_name.eq(Some(file_name)),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
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
        let mut conn = self.pool.get().await?;
        diesel::update(users::table.filter(users::telegram_id.eq(telegram_id)))
            .set((
                users::email_address.eq(Some(email)),
                users::imap_host.eq(Some(imap_host)),
                users::imap_port.eq(Some(imap_port)),
                users::imap_user.eq(Some(imap_user)),
                users::imap_password.eq(Some(imap_pass)),
                users::smtp_host.eq(Some(smtp_host)),
                users::smtp_port.eq(Some(smtp_port)),
            ))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn save_vacancy(&self, vacancy: NewVacancy<'_>) -> AppResult<bool> {
        let mut conn = self.pool.get().await?;
        let rows_affected = diesel::insert_into(vacancies::table)
            .values(&vacancy)
            .on_conflict(vacancies::id)
            .do_nothing()
            .execute(&mut conn)
            .await?;

        Ok(rows_affected > 0)
    }

    pub async fn get_vacancy(&self, vacancy_id: &str) -> AppResult<Option<VacancyDb>> {
        let mut conn = self.pool.get().await?;
        let v = vacancies::table
            .filter(vacancies::id.eq(vacancy_id))
            .first(&mut conn)
            .await
            .optional()?;
        Ok(v)
    }

    pub async fn get_recent_vacancies(
        &self,
        keyword: Option<Keyword>,
        limit: i64,
    ) -> AppResult<Vec<VacancyDb>> {
        let mut conn = self.pool.get().await?;
        let mut query = vacancies::table
            .into_boxed()
            .order(vacancies::discovered_at.desc())
            .limit(limit);

        if let Some(kw) = keyword {
            query = query.filter(vacancies::keyword.eq(kw.as_str()));
        }

        let list = query.load(&mut conn).await?;
        Ok(list)
    }

    pub async fn add_application(
        &self,
        user_id: i64,
        vacancy_id: &str,
        status: &str,
    ) -> AppResult<()> {
        let mut conn = self.pool.get().await?;
        let new_app = NewApplication {
            user_id,
            vacancy_id,
            status,
        };

        diesel::insert_into(applications::table)
            .values(&new_app)
            .on_conflict((applications::user_id, applications::vacancy_id))
            .do_update()
            .set((
                applications::status.eq(status),
                applications::updated_at.eq(chrono::Utc::now()),
            ))
            .execute(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn get_user_applications(
        &self,
        user_id: i64,
    ) -> AppResult<Vec<(ApplicationDb, VacancyDb)>> {
        let mut conn = self.pool.get().await?;
        let list = applications::table
            .inner_join(vacancies::table.on(applications::vacancy_id.eq(vacancies::id)))
            .filter(applications::user_id.eq(user_id))
            .order(applications::applied_at.desc())
            .select((ApplicationDb::as_select(), VacancyDb::as_select()))
            .load::<(ApplicationDb, VacancyDb)>(&mut conn)
            .await?;

        Ok(list)
    }

    pub async fn add_blacklist(&self, user_id: i64, company_name: &str) -> AppResult<()> {
        let mut conn = self.pool.get().await?;
        let new_item = NewBlacklist {
            user_id,
            company_name,
        };

        diesel::insert_into(blacklist::table)
            .values(&new_item)
            .on_conflict((blacklist::user_id, blacklist::company_name))
            .do_nothing()
            .execute(&mut conn)
            .await?;

        Ok(())
    }

    pub async fn remove_blacklist(&self, user_id: i64, company_name: &str) -> AppResult<()> {
        let mut conn = self.pool.get().await?;
        diesel::delete(blacklist::table)
            .filter(blacklist::user_id.eq(user_id))
            .filter(blacklist::company_name.ilike(company_name))
            .execute(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn get_user_blacklist(&self, user_id: i64) -> AppResult<Vec<BlacklistDb>> {
        let mut conn = self.pool.get().await?;
        let list = blacklist::table
            .filter(blacklist::user_id.eq(user_id))
            .order(blacklist::company_name.asc())
            .load::<BlacklistDb>(&mut conn)
            .await?;
        Ok(list)
    }

    pub async fn is_company_blacklisted(
        &self,
        user_id: i64,
        company_name: &str,
    ) -> AppResult<bool> {
        let mut conn = self.pool.get().await?;
        let count: i64 = blacklist::table
            .filter(blacklist::user_id.eq(user_id))
            .filter(blacklist::company_name.ilike(company_name.trim()))
            .count()
            .get_result(&mut conn)
            .await?;

        Ok(count > 0)
    }
}
