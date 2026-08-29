use job_notifier_bot::services::djinni::service::DjinniClient;
use job_notifier_bot::services::dou::service::DouClient;
use job_notifier_bot::services::linkedin::service::LinkedInClient;
use job_notifier_bot::services::storage::models::Keyword;

#[test]
fn test_djinni_parser_sample_html() {
    let html = r#"
    <li class="list-jobs__item">
        <div class="job-list-item__title">
            <a href="/jobs/12345-senior-rust-engineer/">Senior Rust Engineer</a>
        </div>
        <div class="list-jobs__details">
            <a href="/jobs/company-cooltech/">CoolTech</a>
            <span class="public-salary-item">$5000 - $7000</span>
            <div class="job-list-item__tags">
                <span>Rust</span>
                <span>Tokio</span>
                <span>PostgreSQL</span>
            </div>
        </div>
        <div class="job-list-item__description">
            We are looking for a Senior Rust Engineer to join our high-load fintech team.
        </div>
    </li>
    "#;

    let client = DjinniClient::new();
    let vacancies = client.parse_html(html, Keyword::Rust).expect("Parse error");

    assert_eq!(vacancies.len(), 1);
    let vac = &vacancies[0];
    assert_eq!(vac.title, "Senior Rust Engineer");
    assert_eq!(vac.company, "CoolTech");
    assert_eq!(vac.salary.as_deref(), Some("$5000 - $7000"));
    assert_eq!(vac.stack.as_deref(), Some("Rust, Tokio, PostgreSQL"));
    assert!(vac.url.contains("12345-senior-rust-engineer"));
    assert!(vac.summary.contains("Senior Rust Engineer to join"));
}

#[test]
fn test_djinni_parser_modern_html() {
    let html = r#"
    <div id="job-item-818600" class="job-item card-link fs-5 mb-4 rounded-2 p-2">
      <div class="d-flex flex-column gap-1">
        <a href="/jobs/818600-rust-developer/" class="job_item__header-link d-flex flex-column gap-1 text-decoration-none">
          <header class="row gx-2 align-items-start">
            <div class="col">
              <h2 class="job-item__position fs-4 m-0 mb-1">Rust Developer</h2>
              <div class="d-flex flex-wrap align-items-center column-gap-1">
                <span class="small text-gray-800 opacity-75 font-weight-500">Quantum Systems Ukraine</span>
              </div>
            </div>
            <div class="col-auto">
              <span class="text-body-tertiary fw-medium" title="Рівень зарплати: $$$">$$$</span>
            </div>
          </header>
        </a>
        <div class="job-item__tags">
          <span class="badge text-uppercase">🪖 DefTech</span>
          <span class="badge text-bg-light">Продуктова компанія</span>
        </div>
        <div id="job-description-818600">
          <span class="js-truncated-text">The role is based in the Kyiv region, and we will expect you to work full-time...</span>
        </div>
      </div>
    </div>
    "#;

    let client = DjinniClient::new();
    let vacancies = client.parse_html(html, Keyword::Rust).expect("Parse error");

    assert_eq!(vacancies.len(), 1);
    let vac = &vacancies[0];
    assert_eq!(vac.title, "Rust Developer");
    assert_eq!(vac.company, "Quantum Systems Ukraine");
    assert_eq!(vac.salary.as_deref(), Some("$$$"));
    assert_eq!(
        vac.stack.as_deref(),
        Some("🪖 DefTech, Продуктова компанія")
    );
    assert!(vac.url.contains("818600-rust-developer"));
    assert!(vac.summary.contains("The role is based in the Kyiv region"));
}

#[test]
fn test_dou_parser_sample_html() {
    let html = r#"
    <li class="l-vacancy">
        <a class="vt" href="https://jobs.dou.ua/vacancies/98765/">Go Backend Developer</a>
        <a class="company" href="https://jobs.dou.ua/companies/globex/">Globex Corporation</a>
        <span class="salary">$3500–$5000</span>
        <span class="cities">Kyiv, Remote</span>
        <div class="sh-desc">
            Розробка мікросервісів на Go, робота з Kafka, Kubernetes та gRPC.
        </div>
    </li>
    "#;

    let client = DouClient::new();
    let vacancies = client.parse_html(html, Keyword::Go).expect("Parse error");

    assert_eq!(vacancies.len(), 1);
    let vac = &vacancies[0];
    assert_eq!(vac.title, "Go Backend Developer");
    assert_eq!(vac.company, "Globex Corporation");
    assert_eq!(vac.salary.as_deref(), Some("$3500–$5000"));
    assert_eq!(vac.stack.as_deref(), Some("Kyiv, Remote"));
    assert_eq!(vac.url, "https://jobs.dou.ua/vacancies/98765/");
    assert!(vac.summary.contains("мікросервісів на Go"));
}

#[test]
fn test_linkedin_parser_sample_html() {
    let html = r#"
    <li class="job-search-card">
        <a class="base-card__full-link" href="https://www.linkedin.com/jobs/view/1122334455?refId=abc">
            <h3 class="base-search-card__title">Software Engineer (Rust/Go)</h3>
        </a>
        <h4 class="base-search-card__subtitle">
            <a href="https://www.linkedin.com/company/acme">Acme Corp</a>
        </h4>
        <span class="job-search-card__location">Kyiv, Ukraine (Remote)</span>
    </li>
    "#;

    let client = LinkedInClient::new();
    let vacancies = client.parse_html(html, Keyword::Rust).expect("Parse error");

    assert_eq!(vacancies.len(), 1);
    let vac = &vacancies[0];
    assert_eq!(vac.title, "Software Engineer (Rust/Go)");
    assert_eq!(vac.company, "Acme Corp");
    assert_eq!(vac.stack.as_deref(), Some("Kyiv, Ukraine (Remote)"));
    assert_eq!(vac.url, "https://www.linkedin.com/jobs/view/1122334455");
}

#[test]
fn test_keyword_matching() {
    // Rust valid matches
    assert!(Keyword::Rust.is_match("Senior Rust Engineer", None, ""));
    assert!(Keyword::Rust.is_match("Backend Developer (Rust/C++)", None, ""));
    assert!(Keyword::Rust.is_match("Core Developer", Some("Rust, Tokio"), ""));
    assert!(Keyword::Rust.is_match("Systems Programmer", None, "Writing code in Rustlang"));

    // Rust false positives (should NOT match)
    assert!(!Keyword::Rust.is_match("AI Engineer", None, "Working on LLMs and Python"));
    assert!(!Keyword::Rust.is_match("Trust & Safety Analyst", None, "Reviewing policy"));
    assert!(!Keyword::Rust.is_match("Robust Systems Architect", None, "Building Java backends"));
    assert!(!Keyword::Rust.is_match("Unreal Engine Developer", None, "Game dev in C++"));

    // Go valid matches
    assert!(Keyword::Go.is_match("Golang Backend Engineer", None, ""));
    assert!(Keyword::Go.is_match("Senior Go Developer", None, ""));
    assert!(Keyword::Go.is_match("Software Engineer (Go)", None, ""));
    assert!(Keyword::Go.is_match("Fullstack Engineer", Some("Go, React"), ""));
    assert!(Keyword::Go.is_match(
        "Backend Engineer",
        None,
        "Developing Go microservices with gRPC"
    ));

    // Go false positives (should NOT match)
    assert!(!Keyword::Go.is_match("Python SDET Engineer", None, "Automated tests in Python"));
    assert!(!Keyword::Go.is_match("Ongoing Project Manager", None, "Managing tasks"));
    assert!(!Keyword::Go.is_match("Algorithm Engineer", None, "Category theory and ML"));
    assert!(!Keyword::Go.is_match(
        "Django Backend Developer",
        None,
        "Python Django development"
    ));
}
