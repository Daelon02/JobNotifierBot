use serde::{Deserialize, Serialize};
use teloxide::dispatching::dialogue::ErasedStorage;
use teloxide::macros::BotCommands;

pub type MyDialogue = teloxide::dispatching::dialogue::Dialogue<State, ErasedStorage<State>>;

#[derive(BotCommands, Clone, Debug)]
#[command(rename_rule = "snake_case", description = "Доступні команди:")]
pub enum Command {
    #[command(description = "Головне меню та статус")]
    Start,
    #[command(description = "Завантажити або переглянути резюме")]
    Cv,
    #[command(description = "Налаштування пошукових фільтрів (Rust / Go)")]
    Filters,
    #[command(description = "Список вакансій, на які ви відгукнулися")]
    Applied,
    #[command(description = "Список збережених вакансій у базі")]
    Vacancies,
    #[command(description = "Керування чорним списком компаній")]
    Blacklist,
    #[command(description = "Налаштування моніторингу пошти для оферів")]
    Email,
    #[command(description = "Скасувати поточну дію")]
    Cancel,
    #[command(description = "Довідка по використанню")]
    Help,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum State {
    #[default]
    Start,
    WaitingForCvDocument,
    WaitingForEmailSetup,
    WaitingForEmailReplyText {
        recipient: String,
        subject: String,
    },
    WaitingForBlacklistInput,
}
