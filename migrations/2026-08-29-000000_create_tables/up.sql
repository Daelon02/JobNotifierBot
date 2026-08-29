CREATE TABLE IF NOT EXISTS users (
    telegram_id BIGINT PRIMARY KEY,
    track_rust BOOLEAN NOT NULL DEFAULT FALSE,
    track_go BOOLEAN NOT NULL DEFAULT FALSE,
    cv_file_path VARCHAR,
    cv_file_name VARCHAR,
    email_address VARCHAR,
    imap_host VARCHAR,
    imap_port INTEGER,
    imap_user VARCHAR,
    imap_password VARCHAR,
    smtp_host VARCHAR,
    smtp_port INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS vacancies (
    id VARCHAR PRIMARY KEY,
    platform VARCHAR NOT NULL,
    keyword VARCHAR NOT NULL,
    title VARCHAR NOT NULL,
    company VARCHAR NOT NULL,
    salary VARCHAR,
    stack VARCHAR,
    summary TEXT NOT NULL,
    url TEXT NOT NULL,
    discovered_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS applications (
    id SERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(telegram_id) ON DELETE CASCADE,
    vacancy_id VARCHAR NOT NULL REFERENCES vacancies(id) ON DELETE CASCADE,
    status VARCHAR NOT NULL DEFAULT 'applied',
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_user_vacancy UNIQUE (user_id, vacancy_id)
);

CREATE TABLE IF NOT EXISTS blacklist (
    id SERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(telegram_id) ON DELETE CASCADE,
    company_name VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT unique_user_company UNIQUE (user_id, company_name)
);

CREATE INDEX IF NOT EXISTS idx_vacancies_discovered_at ON vacancies(discovered_at DESC);
CREATE INDEX IF NOT EXISTS idx_applications_user_id ON applications(user_id);
CREATE INDEX IF NOT EXISTS idx_blacklist_user_company ON blacklist(user_id, company_name);
