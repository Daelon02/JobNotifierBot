// @generated automatically by Diesel CLI.

diesel::table! {
    users (telegram_id) {
        telegram_id -> BigInt,
        track_rust -> Bool,
        track_go -> Bool,
        cv_file_path -> Nullable<VarChar>,
        cv_file_name -> Nullable<VarChar>,
        email_address -> Nullable<VarChar>,
        imap_host -> Nullable<VarChar>,
        imap_port -> Nullable<Int4>,
        imap_user -> Nullable<VarChar>,
        imap_password -> Nullable<VarChar>,
        smtp_host -> Nullable<VarChar>,
        smtp_port -> Nullable<Int4>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    vacancies (id) {
        id -> VarChar,
        platform -> VarChar,
        keyword -> VarChar,
        title -> VarChar,
        company -> VarChar,
        salary -> Nullable<VarChar>,
        stack -> Nullable<VarChar>,
        summary -> Text,
        url -> Text,
        discovered_at -> Timestamptz,
    }
}

diesel::table! {
    applications (id) {
        id -> Int4,
        user_id -> BigInt,
        vacancy_id -> VarChar,
        status -> VarChar,
        applied_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    blacklist (id) {
        id -> Int4,
        user_id -> BigInt,
        company_name -> VarChar,
        created_at -> Timestamptz,
    }
}

diesel::joinable!(applications -> users (user_id));
diesel::joinable!(applications -> vacancies (vacancy_id));
diesel::joinable!(blacklist -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(applications, blacklist, users, vacancies);
