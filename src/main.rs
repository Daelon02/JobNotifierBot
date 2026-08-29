use diesel::Connection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use job_notifier_bot::config::Config;
use job_notifier_bot::error::{AppError, AppResult};
use job_notifier_bot::services::djinni::service::{DjinniClient, run_djinni_polling};
use job_notifier_bot::services::dou::service::{DouClient, run_dou_polling};
use job_notifier_bot::services::email::service::run_email_polling;
use job_notifier_bot::services::linkedin::service::{LinkedInClient, run_linkedin_polling};
use job_notifier_bot::services::storage::service::Db;
use job_notifier_bot::services::telegram::models::State;
use job_notifier_bot::services::telegram::service::schema;
use log::{error, info};
use teloxide::dispatching::dialogue::serializer::Bincode;
use teloxide::dispatching::dialogue::{ErasedStorage, RedisStorage, Storage};
use teloxide::prelude::*;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting Telegram Job Notifier Bot...");

    // Load configuration
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());
    let config = Config::load(&config_path).map_err(|e| {
        error!("Failed to load {}: {:?}", config_path, e);
        e
    })?;

    // Determine PostgreSQL URL
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| config.postgres.url.clone());

    // Run Diesel database migrations synchronously on startup
    info!("Running database migrations...");
    let db_url_clone = database_url.clone();
    tokio::task::spawn_blocking(move || {
        let mut conn = diesel::PgConnection::establish(&db_url_clone)
            .map_err(|e| AppError::Storage(e.into()))?;
        conn.run_pending_migrations(MIGRATIONS)
            .map_err(AppError::Storage)?;
        Ok::<(), AppError>(())
    })
    .await
    .map_err(|e| AppError::Storage(e.into()))??;

    // Connect to PostgreSQL
    info!("Connecting to PostgreSQL database pool...");
    let db = Db::new(&database_url).await?;

    // Connect to Redis for Telegram dialogue session storage
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| config.redis.url.clone());
    info!("Connecting to Redis dialogue storage at {}...", redis_url);
    let storage: std::sync::Arc<ErasedStorage<State>> = RedisStorage::open(&redis_url, Bincode)
        .await
        .map_err(|e| AppError::Storage(e.into()))?
        .erase();

    let bot = Bot::new(&config.telegram.bot_token);
    let djinni_client = DjinniClient::new();
    let dou_client = DouClient::new();
    let linkedin_client = LinkedInClient::new();

    // Build the handler dispatcher
    let mut dispatcher = Dispatcher::builder(bot.clone(), schema())
        .dependencies(dptree::deps![
            db.clone(),
            djinni_client.clone(),
            dou_client.clone(),
            linkedin_client.clone(),
            storage
        ])
        .enable_ctrlc_handler()
        .build();

    let interval_seconds = config.scraping.interval_seconds;

    // Spawn Djinni Scraper Polling Task
    let djinni_db = db.clone();
    let djinni_bot = bot.clone();
    let djinni_c = djinni_client.clone();
    tokio::spawn(async move {
        info!("Spawning Djinni vacancy polling task...");
        if let Err(e) = run_djinni_polling(djinni_c, djinni_db, djinni_bot, interval_seconds).await
        {
            error!("Djinni scraper runner crashed: {:?}", e);
        }
    });

    // Spawn DOU Scraper Polling Task
    let dou_db = db.clone();
    let dou_bot = bot.clone();
    let dou_c = dou_client.clone();
    tokio::spawn(async move {
        info!("Spawning DOU vacancy polling task...");
        if let Err(e) = run_dou_polling(dou_c, dou_db, dou_bot, interval_seconds).await {
            error!("DOU scraper runner crashed: {:?}", e);
        }
    });

    // Spawn LinkedIn Scraper Polling Task
    let linkedin_db = db.clone();
    let linkedin_bot = bot.clone();
    let linkedin_c = linkedin_client.clone();
    tokio::spawn(async move {
        info!("Spawning LinkedIn vacancy polling task...");
        if let Err(e) =
            run_linkedin_polling(linkedin_c, linkedin_db, linkedin_bot, interval_seconds).await
        {
            error!("LinkedIn scraper runner crashed: {:?}", e);
        }
    });

    // Spawn Email Offers Polling Task
    let email_db = db.clone();
    let email_bot = bot.clone();
    tokio::spawn(async move {
        info!("Spawning Email inbox offer polling task...");
        if let Err(e) = run_email_polling(email_db, email_bot, interval_seconds).await {
            error!("Email poller runner crashed: {:?}", e);
        }
    });

    info!("Bot starting...");
    dispatcher.dispatch().await;

    Ok(())
}
