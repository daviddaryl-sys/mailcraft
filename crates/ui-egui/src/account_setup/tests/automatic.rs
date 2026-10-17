use super::*;

fn discoveries(app: &mut MailApp, answer: provider::Discovery) -> Log<String> {
    let asked: Log<String> = Default::default();
    let log = asked.clone();
    app.services.discover_account = Some(Box::new(move |domain| {
        log.lock().unwrap().push(domain);
        let (tx, rx) = mpsc::channel();
        tx.send(Ok(answer.clone())).unwrap();
        rx
    }));
    asked
}
fn dav_jobs(app: &mut MailApp) -> Log<(Id, String, bool)> {
    let asked: Log<(Id, String, bool)> = Default::default();
    let log = asked.clone();
    let keep: Log<mpsc::Sender<mailcraft_model::dav::Download>> = Default::default();
    app.services.dav = Some(Box::new(move |request| {
        log.lock().unwrap().push((request.account, request.password, request.read_only));
        let (tx, rx) = mpsc::channel();
        keep.lock().unwrap().push(tx);
        rx
    }));
    asked
}
fn found() -> provider::Discovery {
    provider::Discovery {
        incoming: Some(Server { host: "imap.example.org".into(), port: 9993, security: Security::Tls }),
        outgoing: Some(Server { host: "smtp.example.org".into(), port: 465, security: Security::Tls }),
        dav_url: "https://group.example.org/dav/".into(),
        ..Default::default()
    }
}
#[test]
fn email_and_one_password_start_mail_calendar_and_contacts_together() {
    let mut a = app(false);
    let asked = discoveries(&mut a, found());
    let tried = checks(&mut a, Some(Ok(())));
    let mail = syncs(&mut a);
    let dav = dav_jobs(&mut a);
    let mut h = open(a, json!({}));
    connect(&mut h, "noor@example.org");
    assert_eq!(page(&h), Some(Page::Password));
    assert_eq!(*asked.lock().unwrap(), ["example.org"]);
    assert!(says(&h, "Connects mail, calendar and contacts together"));
    assert!(wizard(&h).is_some_and(|w| !w.guessed && !w.settings_open));
    type_password(&mut h);
    press(&mut h, Key::Enter);
    assert_eq!(page(&h), Some(Page::Done));
    let account = &h.state().session.mailbox.accounts[0];
    assert_eq!(account.dav_url, "https://group.example.org/dav/");
    assert_eq!(account.incoming.as_ref().unwrap().port, 9993);
    assert_eq!(tried.lock().unwrap()[0].0.dav_url, account.dav_url);
    assert_eq!(mail.lock().unwrap()[0].2, TYPED);
    assert_eq!(*dav.lock().unwrap(), [(account.id, TYPED.into(), true)]);
    assert!(wizard(&h).unwrap().password.0.is_empty());
    assert!(!serde_json::to_string(&h.state().session.mailbox).unwrap().contains(TYPED));
}
#[test]
fn despres_profile_bypasses_legacy_mx_and_never_selects_plesk_imap() {
    let mut a = app(false);
    let asked = discoveries(&mut a, found());
    let mut w = AccountSetup::new(&a, &json!({"email":"setup-check@despresai.com"}));
    w.act(&mut a, Action::Go);
    assert_eq!(w.page, Page::Password);
    assert_eq!(w.service, "Despres Connect");
    assert_eq!(w.account().unwrap().incoming.unwrap().port, 9933);
    assert_eq!(w.dav_url, "https://outlook-test.computerhelpexpert.co.uk/dav/");
    assert!(asked.lock().unwrap().is_empty());
    w.act(&mut a, Action::Back);
    w.email = "setup-check@gmail.com".into();
    w.act(&mut a, Action::Go);
    assert!(w.dav_url.is_empty());
}
#[test]
fn cancelling_discovery_and_reopening_an_existing_account_start_nothing() {
    let mut a = app(false);
    let (tx, rx) = mpsc::channel();
    let rx = Mutex::new(Some(rx));
    a.services.discover_account = Some(Box::new(move |_| rx.lock().unwrap().take().unwrap()));
    let mut w = AccountSetup::new(&a, &json!({"email":"noor@example.org"}));
    w.act(&mut a, Action::Go);
    assert_eq!(w.page, Page::Detecting);
    w.act(&mut a, Action::Back);
    assert!(!w.busy());
    assert!(tx.send(Ok(found())).is_err());
    a.execute("account.add", json!({"email":"noor@example.org"})).unwrap();
    w.act(&mut a, Action::Go);
    assert_eq!(w.page, Page::Address);
    assert!(w.error.contains("already connected"));
    assert_eq!(a.session.mailbox.accounts.len(), 1);
}
#[test]
fn dns_does_not_replace_a_known_oauth_provider_or_silently_claim_missing_services() {
    let mut a = app(false);
    a.services.microsoft_client_id = APP_ID.into();
    sign_ins(&mut a, None);
    let mut result = found();
    result.mx = vec![MICROSOFT_MX.into()];
    discoveries(&mut a, result);
    let mut h = open(a, json!({}));
    connect(&mut h, "noor@example.org");
    h.run_steps(2);
    let w = wizard(&h).unwrap();
    assert!(w.oauth.is_some() && w.dav_url.is_empty());
    assert_eq!(w.incoming.host, "outlook.office365.com");
}
