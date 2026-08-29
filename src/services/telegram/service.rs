use super::models::{Command, MyDialogue, State};
use crate::services::djinni::service::html_escape;
use crate::services::email::service::EmailService;
use crate::services::storage::models::Keyword;
use crate::services::storage::service::Db;
use log::info;
use std::path::Path;
use teloxide::dispatching::dialogue::ErasedStorage;
use teloxide::dispatching::{UpdateFilterExt, UpdateHandler};
use teloxide::dptree;
use teloxide::net::Download;
use teloxide::prelude::*;
use teloxide::types::{
    InlineKeyboardButton, InlineKeyboardMarkup, InputFile, KeyboardButton, KeyboardMarkup,
    LinkPreviewOptions, Message, Update,
};
use tokio::fs::File;

const RESUMES_STORAGE_DIR: &str = "./storage/resumes";

pub fn schema() -> UpdateHandler<Box<dyn std::error::Error + Send + Sync + 'static>> {
    use dptree::case;

    let command_handler = teloxide::filter_command::<Command, _>()
        .branch(case![Command::Start].endpoint(handle_start))
        .branch(case![Command::Cv].endpoint(handle_cv_command))
        .branch(case![Command::Filters].endpoint(handle_filters_command))
        .branch(case![Command::Applied].endpoint(handle_applied_command))
        .branch(case![Command::Vacancies].endpoint(handle_vacancies_command))
        .branch(case![Command::Blacklist].endpoint(handle_blacklist_command))
        .branch(case![Command::Email].endpoint(handle_email_command))
        .branch(case![Command::Cancel].endpoint(handle_cancel_command))
        .branch(case![Command::Help].endpoint(handle_help_command));

    let message_handler = Update::filter_message()
        .enter_dialogue::<Message, ErasedStorage<State>, State>()
        .branch(command_handler)
        .branch(case![State::Start].endpoint(handle_text_menu))
        .branch(case![State::WaitingForCvDocument].endpoint(handle_cv_upload))
        .branch(case![State::WaitingForEmailSetup].endpoint(handle_email_input))
        .branch(
            case![State::WaitingForEmailReplyText { recipient, subject }]
                .endpoint(handle_email_reply_input),
        )
        .branch(case![State::WaitingForBlacklistInput].endpoint(handle_blacklist_input));

    let callback_query_handler = Update::filter_callback_query()
        .enter_dialogue::<CallbackQuery, ErasedStorage<State>, State>()
        .endpoint(handle_callback_query);

    dptree::entry()
        .branch(message_handler)
        .branch(callback_query_handler)
}

fn get_main_menu_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new("📄 Моє резюме"),
            KeyboardButton::new("⚙️ Фільтри мов"),
        ],
        vec![
            KeyboardButton::new("📁 Збережені вакансії"),
            KeyboardButton::new("📋 Мої відгуки"),
        ],
        vec![
            KeyboardButton::new("🚫 Чорний список"),
            KeyboardButton::new("📬 Пошта та офери"),
        ],
        vec![KeyboardButton::new("ℹ️ Довідка")],
    ])
    .resize_keyboard()
}

async fn handle_start(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    if user_id == 0 {
        return Ok(());
    }

    let user = db.get_or_create_user(user_id).await?;
    dialogue.update(State::Start).await?;

    let cv_status = if user.cv_file_name.is_some() {
        "✅ Завантажено"
    } else {
        "⚠️ Не завантажено (/cv для додавання)"
    };

    let email_status = if user.email_address.is_some() {
        "✅ Налаштовано"
    } else {
        "⚪️ Не налаштовано (/email за бажанням)"
    };

    let text = format!(
        "👋 <b>Вітаю у Job Notifier Bot!</b>\n\n\
         Я моніторю нові вакансії кожні 10 хвилин на <b>Djinni</b>, <b>DOU</b> та <b>LinkedIn</b>.\n\n\
         📌 <b>Поточний статус:</b>\n\
         • Фільтр Rust: {}\n\
         • Фільтр Go: {}\n\
         • Резюме (CV): {}\n\
         • Моніторинг пошти: {}\n\n\
         Використовуйте кнопки меню нижче для керування ботом.",
        if user.track_rust {
            "🟢 Увімкнено"
        } else {
            "🔴 Вимкнено"
        },
        if user.track_go {
            "🟢 Увімкнено"
        } else {
            "🔴 Вимкнено"
        },
        cv_status,
        email_status
    );

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(get_main_menu_keyboard())
        .await?;

    Ok(())
}

async fn handle_cv_command(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = db.get_or_create_user(user_id).await?;

    let text = if let Some(filename) = &user.cv_file_name {
        format!(
            "📄 <b>Ваше збережене резюме:</b>\n<code>{}</code>\n\n\
             Щоб оновити його, натисніть кнопку нижче або надішліть новий файл документа (PDF / DOCX).\n\
             <i>Попередній файл буде автоматично видалено.</i>",
            html_escape(filename)
        )
    } else {
        "📄 <b>Резюме ще не завантажено!</b>\n\n\
         Надішліть файл вашого резюме (PDF або DOCX) як документ у чат."
            .to_string()
    };

    dialogue.update(State::WaitingForCvDocument).await?;

    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "📤 Надіслати новий файл резюме",
        "action:upload_cv",
    )]]);

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_filters_command(
    bot: Bot,
    msg: Message,
    _dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = db.get_or_create_user(user_id).await?;

    let text = "⚙️ <b>Пошукові фільтри за мовами:</b>\n\n\
                Оберіть, які сповіщення про вакансії ви хочете отримувати:";

    let rust_btn = if user.track_rust {
        InlineKeyboardButton::callback("🟢 Rust: Увімкнено", "toggle:rust")
    } else {
        InlineKeyboardButton::callback("🔴 Rust: Вимкнено", "toggle:rust")
    };

    let go_btn = if user.track_go {
        InlineKeyboardButton::callback("🟢 Go: Увімкнено", "toggle:go")
    } else {
        InlineKeyboardButton::callback("🔴 Go: Вимкнено", "toggle:go")
    };

    let keyboard = InlineKeyboardMarkup::new(vec![vec![rust_btn], vec![go_btn]]);

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_applied_command(
    bot: Bot,
    msg: Message,
    _dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let apps = db.get_user_applications(user_id).await?;

    if apps.is_empty() {
        bot.send_message(
            msg.chat.id,
            "📋 <b>У вас ще немає відправлених відгуків.</b>\n\n\
             Коли ви натиснете «Надіслати резюме» під новою вакансією, вона автоматично з'явиться тут.",
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    let mut text = "📋 <b>Список ваших відгуків на вакансії:</b>\n\n".to_string();

    for (app, vac) in apps.iter().take(20) {
        let status_emoji = match app.status.as_str() {
            "applied" => "📤 Надіслано",
            "viewed" => "👀 Переглянуто",
            "replied" => "💬 Отримано відповідь",
            "offer" => "🎉 Офер",
            "rejected" => "❌ Відхилено",
            _ => "📌 Надіслано",
        };

        let date_str = app.applied_at.format("%d.%m.%Y %H:%M").to_string();

        text.push_str(&format!(
            "🏢 <b>{}</b> — <a href=\"{}\">{}</a>\n\
             Статус: <b>{}</b> | Дата: <i>{}</i>\n\n",
            html_escape(&vac.company),
            vac.url,
            html_escape(&vac.title),
            status_emoji,
            date_str
        ));
    }

    let link_preview = LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    };

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .link_preview_options(link_preview)
        .await?;

    Ok(())
}

async fn handle_blacklist_command(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let items = db.get_user_blacklist(user_id).await?;

    let mut text = "🚫 <b>Чорний список роботодавців:</b>\n\n".to_string();

    if items.is_empty() {
        text.push_str(
            "Список порожній. Вакансії від усіх компаній відображаються у звичайному режимі.\n\n",
        );
    } else {
        text.push_str("Вакансії від цих компаній не надсилатимуться вам:\n");
        for item in &items {
            text.push_str(&format!("• <b>{}</b>\n", html_escape(&item.company_name)));
        }
        text.push('\n');
    }

    let mut buttons = Vec::new();

    for item in items.iter().take(10) {
        let cb = format!("unbl:{}", item.company_name);
        buttons.push(vec![InlineKeyboardButton::callback(
            format!("❌ Видалити: {}", item.company_name),
            cb,
        )]);
    }

    buttons.push(vec![InlineKeyboardButton::callback(
        "➕ Додати компанію вручну",
        "action:add_bl",
    )]);

    dialogue.update(State::Start).await?;

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(buttons))
        .await?;

    Ok(())
}

async fn handle_email_command(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let user = db.get_or_create_user(user_id).await?;

    let text = if let Some(addr) = &user.email_address {
        format!(
            "📬 <b>Моніторинг пошти підключено:</b>\n<code>{}</code>\n\
             Сервер IMAP: <code>{}</code>\n\
             Сервер SMTP: <code>{}</code>\n\n\
             Бот перевіряє вхідні листи на наявність оферів та запрошень на інтерв'ю.",
            html_escape(addr),
            html_escape(user.imap_host.as_deref().unwrap_or("-")),
            html_escape(user.smtp_host.as_deref().unwrap_or("-"))
        )
    } else {
        "📬 <b>Моніторинг пошти не налаштовано</b> (опціонально).\n\n\
         Якщо ви хочете отримувати сповіщення про офери та листи від рекрутерів прямо в бот,\n\
         натисніть кнопку нижче для швидкого налаштування."
            .to_string()
    };

    let keyboard = InlineKeyboardMarkup::new(vec![vec![InlineKeyboardButton::callback(
        "⚙️ Налаштувати пошту",
        "action:setup_email",
    )]]);

    dialogue.update(State::Start).await?;

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_cancel_command(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dialogue.update(State::Start).await?;
    bot.send_message(
        msg.chat.id,
        "👌 Дію скасовано. Ви повернулися до головного меню.",
    )
    .reply_markup(get_main_menu_keyboard())
    .await?;
    Ok(())
}

async fn handle_help_command(
    bot: Bot,
    msg: Message,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = "ℹ️ <b>Job Notifier Bot — Довідка:</b>\n\n\
         🔹 <b>Моніторинг:</b> Кожні 10 хвилин бот сканує Djinni, DOU та LinkedIn за ключовими словами Rust та Go.\n\
         🔹 <b>Резюме:</b> Завантажте CV через кнопку «Моє резюме» або команду /cv. При відгуку на вакансію воно автоматично відправляється/фіксується.\n\
         🔹 <b>Фільтри:</b> Ви можете у будь-який момент вимкнути або увімкнути мови Rust / Go.\n\
         🔹 <b>Чорний список:</b> Додайте нецікаві компанії в блекліст — сповіщення про їхні вакансії більше не приходитимуть.\n\
         🔹 <b>Пошта:</b> Можливість відслідковувати вхідні офери з вашої скриньки та відповідати прямо з Telegram.";

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
    Ok(())
}

async fn handle_vacancies_command(
    bot: Bot,
    msg: Message,
    _dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let vacancies = db.get_recent_vacancies(None, 15).await?;

    if vacancies.is_empty() {
        bot.send_message(
            msg.chat.id,
            "📁 <b>Збережених вакансій у базі поки немає.</b>\n\n\
             Увімкніть пошук у меню «⚙️ Фільтри мов», і бот одразу знайде та збереже актуальні вакансії з Djinni, DOU та LinkedIn.",
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    let mut text = format!(
        "📁 <b>Збережені вакансії (останні {}):</b>\n\n",
        vacancies.len()
    );

    for (i, vac) in vacancies.iter().enumerate() {
        let tag = match vac.keyword.as_str() {
            "Rust" => "🦀 Rust",
            "Go" => "🐹 Go",
            _ => vac.keyword.as_str(),
        };

        let salary_part = vac
            .salary
            .as_ref()
            .map(|s| format!(" | 💰 {}", html_escape(s)))
            .unwrap_or_default();

        let date_str = vac.discovered_at.format("%d.%m %H:%M").to_string();

        text.push_str(&format!(
            "{}. [<b>{}</b>] <b>{}</b>\n\
             🏢 {} ({}{})\n\
             🕒 <i>{}</i> ➡️ <a href=\"{}\">Переглянути вакансію</a>\n\n",
            i + 1,
            html_escape(&vac.platform.to_uppercase()),
            html_escape(&vac.title),
            html_escape(&vac.company),
            tag,
            salary_part,
            date_str,
            vac.url
        ));
    }

    let keyboard = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("🔥 Всі", "vac_filter:all"),
        InlineKeyboardButton::callback("🦀 Тільки Rust", "vac_filter:rust"),
        InlineKeyboardButton::callback("🐹 Тільки Go", "vac_filter:go"),
    ]]);

    let link_preview = LinkPreviewOptions {
        is_disabled: true,
        url: None,
        prefer_small_media: false,
        prefer_large_media: false,
        show_above_text: false,
    };

    bot.send_message(msg.chat.id, text)
        .parse_mode(teloxide::types::ParseMode::Html)
        .link_preview_options(link_preview)
        .reply_markup(keyboard)
        .await?;

    Ok(())
}

async fn handle_text_menu(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let text = msg.text().unwrap_or_default();

    match text {
        "📄 Моє резюме" => handle_cv_command(bot, msg, dialogue, db).await,
        "⚙️ Фільтри мов" => handle_filters_command(bot, msg, dialogue, db).await,
        "📁 Збережені вакансії" => {
            handle_vacancies_command(bot, msg, dialogue, db).await
        }
        "📋 Мої відгуки" => handle_applied_command(bot, msg, dialogue, db).await,
        "🚫 Чорний список" => handle_blacklist_command(bot, msg, dialogue, db).await,
        "📬 Пошта та офери" => handle_email_command(bot, msg, dialogue, db).await,
        "ℹ️ Довідка" => handle_help_command(bot, msg).await,
        _ => {
            // Check if document was uploaded directly without command
            if msg.document().is_some() {
                handle_cv_upload(bot, msg, dialogue, db).await
            } else {
                bot.send_message(
                    msg.chat.id,
                    "Оберіть пункт меню або введіть /help для довідки.",
                )
                .reply_markup(get_main_menu_keyboard())
                .await?;
                Ok(())
            }
        }
    }
}

async fn handle_cv_upload(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let doc = match msg.document() {
        Some(d) => d,
        None => {
            bot.send_message(
                msg.chat.id,
                "⚠️ Будь ласка, надішліть резюме саме як <b>файл-документ</b> (PDF або DOCX), або введіть /cancel.",
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
            return Ok(());
        }
    };

    tokio::fs::create_dir_all(RESUMES_STORAGE_DIR).await?;

    let file_name = doc
        .file_name
        .clone()
        .unwrap_or_else(|| format!("cv_{}.pdf", user_id));

    let user_existing = db.get_user(user_id).await?;
    if let Some(old_path) = user_existing
        .and_then(|u| u.cv_file_path)
        .filter(|p| Path::new(p).exists())
    {
        let _ = tokio::fs::remove_file(&old_path).await;
        info!("Deleted old CV file: {}", old_path);
    }

    let save_path = format!("{}/{}_{}", RESUMES_STORAGE_DIR, user_id, file_name);

    let tg_file = bot.get_file(doc.file.id.clone()).await?;
    let mut dst = File::create(&save_path).await?;
    bot.download_file(&tg_file.path, &mut dst).await?;

    db.update_user_cv(user_id, &save_path, &file_name).await?;
    dialogue.update(State::Start).await?;

    bot.send_message(
        msg.chat.id,
        format!(
            "🎉 <b>Резюме успішно збережено!</b>\n\nФайл: <code>{}</code>\n\
             Тепер ви можете відгукуватись на вакансії в один клік кнопкою «📩 Надіслати резюме».",
            html_escape(&file_name)
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .reply_markup(get_main_menu_keyboard())
    .await?;

    Ok(())
}

async fn handle_email_input(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let text = msg.text().unwrap_or_default().trim();

    if text.starts_with('/') {
        return Ok(());
    }

    // Format: email, imap_host, imap_port, imap_user, imap_password, smtp_host, smtp_port
    let parts: Vec<&str> = text.split(',').map(|s| s.trim()).collect();

    if parts.len() < 7 {
        bot.send_message(
            msg.chat.id,
            "⚠️ <b>Неправильний формат!</b>\n\n\
             Введіть дані <b>через кому</b> у наступному форматі:\n\
             <code>email, imap_host, imap_port, imap_user, password, smtp_host, smtp_port</code>\n\n\
             🔑 <b>Де взяти пароль (password)?</b>\n\
             Для безпеки використовується <b>Пароль додатку (App Password)</b>, а не основний пароль:\n\
             • <b>Gmail:</b> Акаунт Google ➡️ Безпека ➡️ Двоетапна перевірка ➡️ Паролі додатків (створіть новий та скопіюйте 16 символів).\n\
             • <b>Ukr.net:</b> Налаштування пошти ➡️ Керування доступом IMAP/SMTP ➡️ Створити пароль програми.\n\n\
             📋 <b>Готові шаблони для копіювання:</b>\n\n\
             🔴 <i>Для Gmail:</i>\n\
             <code>myemail@gmail.com, imap.gmail.com, 993, myemail@gmail.com, YOUR_APP_PASSWORD, smtp.gmail.com, 465</code>\n\n\
             🟡 <i>Для Ukr.net:</i>\n\
             <code>myemail@ukr.net, imap.ukr.net, 993, myemail@ukr.net, YOUR_APP_PASSWORD, smtp.ukr.net, 465</code>\n\n\
             (Введіть /cancel для повернення до меню)",
        )
        .parse_mode(teloxide::types::ParseMode::Html)
        .await?;
        return Ok(());
    }

    let email = parts[0];
    let imap_host = parts[1];
    let imap_port = parts[2].parse::<i32>().unwrap_or(993);
    let imap_user = parts[3];
    let imap_password = parts[4];
    let smtp_host = parts[5];
    let smtp_port = parts[6].parse::<i32>().unwrap_or(465);

    db.update_user_email(
        user_id,
        email,
        imap_host,
        imap_port,
        imap_user,
        imap_password,
        smtp_host,
        smtp_port,
    )
    .await?;

    dialogue.update(State::Start).await?;

    bot.send_message(
        msg.chat.id,
        format!(
            "✅ <b>Налаштування пошти успішно збережено!</b>\n\n\
             Пошта: <code>{}</code>\n\
             Бот автоматично відстежуватиме запрошення та офери.",
            html_escape(email)
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .reply_markup(get_main_menu_keyboard())
    .await?;

    Ok(())
}

async fn handle_email_reply_input(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
    (recipient, subject): (String, String),
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let text = msg.text().unwrap_or_default().trim();

    if text.starts_with('/') {
        return Ok(());
    }

    let user = match db.get_user(user_id).await? {
        Some(u) => u,
        None => return Ok(()),
    };

    bot.send_message(msg.chat.id, "⏳ Надсилаю вашу відповідь...")
        .await?;

    match EmailService::send_reply(&user, &recipient, &subject, text).await {
        Ok(_) => {
            dialogue.update(State::Start).await?;
            bot.send_message(
                msg.chat.id,
                format!(
                    "🎉 <b>Лист успішно надіслано!</b>\n\nОдержувач: <code>{}</code>",
                    html_escape(&recipient)
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .reply_markup(get_main_menu_keyboard())
            .await?;
        }
        Err(e) => {
            bot.send_message(
                msg.chat.id,
                format!(
                    "❌ <b>Помилка надсилання листа:</b> {:?}\n\nПеревірте налаштування SMTP у /email.",
                    e
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
    }

    Ok(())
}

async fn handle_blacklist_input(
    bot: Bot,
    msg: Message,
    dialogue: MyDialogue,
    db: Db,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = msg.from.as_ref().map(|u| u.id.0 as i64).unwrap_or(0);
    let company_name = msg.text().unwrap_or_default().trim();

    if company_name.starts_with('/') {
        return Ok(());
    }

    db.add_blacklist(user_id, company_name).await?;
    dialogue.update(State::Start).await?;

    bot.send_message(
        msg.chat.id,
        format!(
            "🚫 Компанію <b>{}</b> успішно додано до чорного списку.",
            html_escape(company_name)
        ),
    )
    .parse_mode(teloxide::types::ParseMode::Html)
    .reply_markup(get_main_menu_keyboard())
    .await?;

    Ok(())
}

async fn handle_callback_query(
    bot: Bot,
    q: CallbackQuery,
    dialogue: MyDialogue,
    db: Db,
    djinni: crate::services::djinni::service::DjinniClient,
    dou: crate::services::dou::service::DouClient,
    linkedin: crate::services::linkedin::service::LinkedInClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = match q.data.as_deref() {
        Some(d) => d,
        None => return Ok(()),
    };

    let user_id = q.from.id.0 as i64;
    let chat_id = q.message.as_ref().map(|m| m.chat().id);

    if let Some(key) = data.strip_prefix("toggle:") {
        let user = db.get_or_create_user(user_id).await?;
        let mut enabled_keyword: Option<Keyword> = None;

        match key {
            "rust" => {
                let new_val = !user.track_rust;
                db.toggle_keyword(user_id, Keyword::Rust, new_val).await?;
                if new_val {
                    enabled_keyword = Some(Keyword::Rust);
                }
            }
            "go" => {
                let new_val = !user.track_go;
                db.toggle_keyword(user_id, Keyword::Go, new_val).await?;
                if new_val {
                    enabled_keyword = Some(Keyword::Go);
                }
            }
            _ => {}
        }

        let updated_user = db.get_or_create_user(user_id).await?;

        let rust_btn = if updated_user.track_rust {
            InlineKeyboardButton::callback("🟢 Rust: Увімкнено", "toggle:rust")
        } else {
            InlineKeyboardButton::callback("🔴 Rust: Вимкнено", "toggle:rust")
        };

        let go_btn = if updated_user.track_go {
            InlineKeyboardButton::callback("🟢 Go: Увімкнено", "toggle:go")
        } else {
            InlineKeyboardButton::callback("🔴 Go: Вимкнено", "toggle:go")
        };

        let keyboard = InlineKeyboardMarkup::new(vec![vec![rust_btn], vec![go_btn]]);

        if let Some(msg) = q.message {
            bot.edit_message_reply_markup(msg.chat().id, msg.id())
                .reply_markup(keyboard)
                .await?;
        }

        if let Some(kw) = enabled_keyword {
            bot.answer_callback_query(q.id)
                .text(format!(
                    "Пошук {} увімкнено! Паршу вакансії...",
                    kw.display_name()
                ))
                .await?;

            if let Some(c_id) = chat_id {
                bot.send_message(
                    c_id,
                    format!(
                        "🔍 <b>Пошук #{} увімкнено!</b>\nПаршу актуальні вакансії на Djinni, DOU та LinkedIn...",
                        kw.display_name()
                    ),
                )
                .parse_mode(teloxide::types::ParseMode::Html)
                .await?;

                let bot_clone = bot.clone();
                let db_clone = db.clone();
                let djinni_clone = djinni.clone();
                let dou_clone = dou.clone();
                let linkedin_clone = linkedin.clone();

                tokio::spawn(async move {
                    trigger_immediate_search_and_send(
                        bot_clone,
                        db_clone,
                        djinni_clone,
                        dou_clone,
                        linkedin_clone,
                        user_id,
                        kw,
                    )
                    .await;
                });
            }
        } else {
            bot.answer_callback_query(q.id)
                .text("Пошук вимкнено!")
                .await?;
        }

        return Ok(());
    }

    if let Some(filter_type) = data.strip_prefix("vac_filter:") {
        let (kw_filter, title_str) = match filter_type {
            "rust" => (Some(Keyword::Rust), "Rust"),
            "go" => (Some(Keyword::Go), "Go"),
            _ => (None, "всі мови"),
        };

        let vacancies = db.get_recent_vacancies(kw_filter, 15).await?;

        let mut text = if vacancies.is_empty() {
            format!(
                "📁 <b>Збережених вакансій за фільтром «{}» не знайдено.</b>",
                title_str
            )
        } else {
            format!(
                "📁 <b>Збережені вакансії ({} — останні {}):</b>\n\n",
                title_str,
                vacancies.len()
            )
        };

        for (i, vac) in vacancies.iter().enumerate() {
            let tag = match vac.keyword.as_str() {
                "Rust" => "🦀 Rust",
                "Go" => "🐹 Go",
                _ => vac.keyword.as_str(),
            };

            let salary_part = vac
                .salary
                .as_ref()
                .map(|s| format!(" | 💰 {}", html_escape(s)))
                .unwrap_or_default();

            let date_str = vac.discovered_at.format("%d.%m %H:%M").to_string();

            text.push_str(&format!(
                "{}. [<b>{}</b>] <b>{}</b>\n\
                 🏢 {} ({}{})\n\
                 🕒 <i>{}</i> ➡️ <a href=\"{}\">Переглянути вакансію</a>\n\n",
                i + 1,
                html_escape(&vac.platform.to_uppercase()),
                html_escape(&vac.title),
                html_escape(&vac.company),
                tag,
                salary_part,
                date_str,
                vac.url
            ));
        }

        let keyboard = InlineKeyboardMarkup::new(vec![vec![
            InlineKeyboardButton::callback("🔥 Всі", "vac_filter:all"),
            InlineKeyboardButton::callback("🦀 Тільки Rust", "vac_filter:rust"),
            InlineKeyboardButton::callback("🐹 Тільки Go", "vac_filter:go"),
        ]]);

        let link_preview = LinkPreviewOptions {
            is_disabled: true,
            url: None,
            prefer_small_media: false,
            prefer_large_media: false,
            show_above_text: false,
        };

        if let Some(msg) = q.message {
            bot.edit_message_text(msg.chat().id, msg.id(), text)
                .parse_mode(teloxide::types::ParseMode::Html)
                .link_preview_options(link_preview)
                .reply_markup(keyboard)
                .await?;
        }

        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }

    if let Some(vacancy_id) = data.strip_prefix("apply:") {
        let user = db.get_or_create_user(user_id).await?;

        match &user.cv_file_path {
            Some(cv_path) => {
                db.add_application(user_id, vacancy_id, "applied").await?;

                if let Some(c_id) = chat_id {
                    let file_path = Path::new(cv_path);
                    if file_path.exists() {
                        let input_file = InputFile::file(file_path);
                        bot.send_document(c_id, input_file)
                            .caption("✅ <b>Ваше резюме надіслано на вакансію!</b>\nСтатус у системі: <code>Надіслано</code>.")
                            .parse_mode(teloxide::types::ParseMode::Html)
                            .await?;
                    } else {
                        bot.send_message(
                            c_id,
                            "✅ <b>Відгук зафіксовано зі статусом 'Надіслано'!</b>",
                        )
                        .parse_mode(teloxide::types::ParseMode::Html)
                        .await?;
                    }
                }

                bot.answer_callback_query(q.id)
                    .text("Резюме надіслано!")
                    .await?;
            }
            None => {
                if let Some(c_id) = chat_id {
                    bot.send_message(
                        c_id,
                        "⚠️ <b>У вас не збережено резюме!</b>\nБудь ласка, завантажте файл резюме командою /cv перед відгуком.",
                    )
                    .parse_mode(teloxide::types::ParseMode::Html)
                    .await?;
                }
                bot.answer_callback_query(q.id)
                    .text("Спочатку завантажте резюме (/cv)")
                    .await?;
            }
        }

        return Ok(());
    }

    if let Some(company) = data.strip_prefix("bl:") {
        db.add_blacklist(user_id, company).await?;

        if let Some(c_id) = chat_id {
            bot.send_message(
                c_id,
                format!(
                    "🚫 Компанію <b>{}</b> додано до чорного списку.\nНові вакансії від неї більше не надходитимуть.",
                    html_escape(company)
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }

        bot.answer_callback_query(q.id)
            .text("Компанію додано до чорного списку!")
            .await?;
        return Ok(());
    }

    if let Some(company) = data.strip_prefix("unbl:") {
        db.remove_blacklist(user_id, company).await?;

        if let Some(c_id) = chat_id {
            bot.send_message(
                c_id,
                format!(
                    "✅ Компанію <b>{}</b> видалено з чорного списку.",
                    html_escape(company)
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }

        bot.answer_callback_query(q.id)
            .text("Видалено з чорного списку!")
            .await?;
        return Ok(());
    }

    if data == "action:upload_cv" {
        dialogue.update(State::WaitingForCvDocument).await?;
        if let Some(c_id) = chat_id {
            bot.send_message(
                c_id,
                "📎 Надішліть файл вашого нового резюме (PDF/DOCX) як документ у чат.",
            )
            .await?;
        }
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }

    if data == "action:add_bl" {
        dialogue.update(State::WaitingForBlacklistInput).await?;
        if let Some(c_id) = chat_id {
            bot.send_message(
                c_id,
                "✍️ Введіть точну або часткову назву компанії для додавання у чорний список:",
            )
            .await?;
        }
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }

    if data == "action:setup_email" {
        dialogue.update(State::WaitingForEmailSetup).await?;
        if let Some(c_id) = chat_id {
            bot.send_message(
                c_id,
                "📧 <b>Налаштування моніторингу пошти (IMAP + SMTP)</b>\n\n\
                 Введіть 7 параметрів <b>через кому</b> у наступному форматі:\n\
                 <code>email, imap_host, imap_port, imap_user, password, smtp_host, smtp_port</code>\n\n\
                 🔍 <b>Де взяти ці параметри?</b>\n\
                 • <b>email:</b> Ваша поштова адреса\n\
                 • <b>imap_host / imap_port:</b> IMAP сервер вхідної пошти (зазвичай порт <code>993</code>)\n\
                 • <b>imap_user:</b> Ваш логін (зазвичай повний email)\n\
                 • <b>password:</b> ⚠️ <b>Пароль додатку (App Password)</b>, згенерований у налаштуваннях безпеки вашого поштового сервісу (не використовуйте основний пароль від акаунту!)\n\
                 • <b>smtp_host / smtp_port:</b> SMTP сервер вихідної пошти для відповідей (зазвичай порт <code>465</code> або <code>587</code>)\n\n\
                 💡 <b>Інструкції для створення App Password:</b>\n\
                 • <b>Gmail:</b> myaccount.google.com ➡️ <i>Безпека</i> ➡️ <i>Двоетапна перевірка</i> ➡️ <i>Паролі додатків</i> (створіть пароль з назвою 'Job Bot' і скопіюйте 16 літер без пробілів).\n\
                 • <b>Ukr.net:</b> mail.ukr.net ➡️ <i>Налаштування</i> ➡️ <i>Керування доступом IMAP/SMTP</i> ➡️ <i>Створити пароль для програм</i>.\n\n\
                 📋 <b>Приклад для копіювання та заміни:</b>\n\
                 <code>candidate@gmail.com, imap.gmail.com, 993, candidate@gmail.com, abcd efgh ijkl mnop, smtp.gmail.com, 465</code>\n\n\
                 <i>(Введіть /cancel якщо хочете повернутися до меню)</i>",
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }

    if let Some(recipient) = data.strip_prefix("reply_email:") {
        let recipient_str = recipient.to_string();
        dialogue
            .update(State::WaitingForEmailReplyText {
                recipient: recipient_str.clone(),
                subject: "Відповідь на пропозицію роботи".to_string(),
            })
            .await?;

        if let Some(c_id) = chat_id {
            bot.send_message(
                c_id,
                format!(
                    "✍️ Введіть текст вашої відповіді на лист для <code>{}</code>:",
                    html_escape(&recipient_str)
                ),
            )
            .parse_mode(teloxide::types::ParseMode::Html)
            .await?;
        }
        bot.answer_callback_query(q.id).await?;
        return Ok(());
    }

    bot.answer_callback_query(q.id).await?;
    Ok(())
}

async fn trigger_immediate_search_and_send(
    bot: Bot,
    db: Db,
    djinni: crate::services::djinni::service::DjinniClient,
    dou: crate::services::dou::service::DouClient,
    linkedin: crate::services::linkedin::service::LinkedInClient,
    user_id: i64,
    keyword: Keyword,
) {
    let mut sent_count = 0;

    // 1. Djinni
    if let Ok(vacancies) = djinni.fetch_vacancies(keyword).await {
        for vac in vacancies.iter().take(5) {
            let new_vac = crate::services::storage::models::NewVacancy {
                id: &vac.id,
                platform: crate::services::storage::models::Platform::Djinni.as_str(),
                keyword: vac.keyword.as_str(),
                title: &vac.title,
                company: &vac.company,
                salary: vac.salary.as_deref(),
                stack: vac.stack.as_deref(),
                summary: &vac.summary,
                url: &vac.url,
            };
            let _ = db.save_vacancy(new_vac).await;
            if let Ok(true) = crate::services::djinni::service::send_djinni_vacancy_to_user(
                &db, &bot, user_id, vac,
            )
            .await
            {
                sent_count += 1;
            }
        }
    }

    // 2. DOU
    if let Ok(vacancies) = dou.fetch_vacancies(keyword).await {
        for vac in vacancies.iter().take(5) {
            let new_vac = crate::services::storage::models::NewVacancy {
                id: &vac.id,
                platform: crate::services::storage::models::Platform::Dou.as_str(),
                keyword: vac.keyword.as_str(),
                title: &vac.title,
                company: &vac.company,
                salary: vac.salary.as_deref(),
                stack: vac.stack.as_deref(),
                summary: &vac.summary,
                url: &vac.url,
            };
            let _ = db.save_vacancy(new_vac).await;
            if let Ok(true) =
                crate::services::dou::service::send_dou_vacancy_to_user(&db, &bot, user_id, vac)
                    .await
            {
                sent_count += 1;
            }
        }
    }

    // 3. LinkedIn
    if let Ok(vacancies) = linkedin.fetch_vacancies(keyword).await {
        for vac in vacancies.iter().take(5) {
            let new_vac = crate::services::storage::models::NewVacancy {
                id: &vac.id,
                platform: crate::services::storage::models::Platform::LinkedIn.as_str(),
                keyword: vac.keyword.as_str(),
                title: &vac.title,
                company: &vac.company,
                salary: vac.salary.as_deref(),
                stack: vac.stack.as_deref(),
                summary: &vac.summary,
                url: &vac.url,
            };
            let _ = db.save_vacancy(new_vac).await;
            if let Ok(true) = crate::services::linkedin::service::send_linkedin_vacancy_to_user(
                &db, &bot, user_id, vac,
            )
            .await
            {
                sent_count += 1;
            }
        }
    }

    let summary_msg = if sent_count > 0 {
        format!(
            "✅ <b>Готово!</b> Надіслано <b>{}</b> свіжих вакансій по #{}.\nНаступні нові вакансії надходитимуть автоматично раз на 10 хвилин.",
            sent_count,
            keyword.display_name()
        )
    } else {
        format!(
            "ℹ️ Наразі нових вакансій по #{} не знайдено. Бот повідомить вас одразу при появі наступних!",
            keyword.display_name()
        )
    };

    let _ = bot
        .send_message(ChatId(user_id), summary_msg)
        .parse_mode(teloxide::types::ParseMode::Html)
        .await;
}
