//! DAV jobs use the same OS-keychain password as the explicitly connected mail account.
use crate::{MailApp, now_ms};
use mailcraft_model::{
    Id,
    dav::{Download, Request},
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
#[derive(Default)]
pub(crate) struct DavSync {
    pub rx: Option<Receiver<Download>>,
    checked: HashMap<Id, (f64, u64)>,
    failures: HashMap<Id, u32>,
    pub status: String,
}
fn retry_interval(normal: f64, failures: u32) -> f64 {
    if failures == 0 { normal } else { (300_000.0 * 2f64.powi(failures.min(4) as i32 - 1)).min(1_800_000.0) }
}

impl MailApp {
    pub(crate) fn start_dav(&mut self, p: &Value) -> Result<Value, String> {
        if self.dav.rx.is_some() {
            return Err(tl!("Calendar and contacts sync is already running").into());
        }
        let account = p
            .get("account")
            .and_then(Value::as_u64)
            .or_else(|| self.session.mailbox.accounts.iter().find(|a| !a.dav_url.is_empty()).map(|a| a.id))
            .ok_or(tl!("Configure a DAV URL in Account Settings first"))?;
        let account = self.session.mailbox.account(account).filter(|a| !a.dav_url.is_empty()).ok_or(tl!("This account has no DAV connection"))?;
        let password = self
            .passwords
            .get(&account.id)
            .cloned()
            .or_else(|| self.remembered("password", &account.email))
            .filter(|p| !p.is_empty())
            .ok_or(tl!("Sign in to this mail account before syncing its calendar and contacts"))?;
        let service = self.services.dav.as_ref().ok_or(tl!("DAV sync is available in the desktop app"))?;
        let id = account.id;
        self.dav.rx = Some(service(Request {
            account: id,
            password,
            mailbox: self.session.mailbox.clone(),
            read_only: p.get("readOnly").and_then(Value::as_bool).unwrap_or(false)
                || !self.session.mailbox.dav_collections.iter().any(|c| c.account == id),
        }));
        self.dav.checked.insert(id, (now_ms(), self.session.rev()));
        self.dav.status = tl!("Syncing calendars and contacts…").into();
        Ok(json!({"pending":"davSync","account":id}))
    }
    pub(crate) fn dav_tick(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.dav.rx {
            match rx.try_recv() {
                Ok(download) => {
                    if let Some((done, total)) = download.progress {
                        if !download.resources.is_empty() || !download.collections.is_empty() {
                            let errors = mailcraft_engine::dav::apply(&mut self.session, download);
                            if !errors.is_empty() {
                                self.status(errors.join("; "));
                            }
                        }
                        self.dav.status = format!("{}: {done} / {total} {}", tl!("Calendar and contacts"), tl!("items checked"));
                        self.status(self.dav.status.clone());
                        ctx.request_repaint();
                        return;
                    }
                    let account = download.account;
                    let resources = download.resources.len();
                    let uploaded = download.uploaded;
                    let mut errors = download.errors.clone();
                    let complete = download.complete;
                    self.dav.rx = None;
                    errors.extend(mailcraft_engine::dav::apply(&mut self.session, download));
                    if complete && errors.is_empty() {
                        self.dav.failures.remove(&account);
                    } else {
                        let failures = self.dav.failures.entry(account).or_default();
                        *failures = failures.saturating_add(1);
                    }
                    self.dav.checked.insert(account, (now_ms(), self.session.rev()));
                    self.dav.status = if errors.is_empty() && complete {
                        format!("{}: {resources} {}; {uploaded} {}", tl!("Calendar and contacts"), tl!("items synced"), tl!("changes confirmed"))
                    } else {
                        errors.join("; ")
                    };
                    self.status(self.dav.status.clone());
                }
                Err(TryRecvError::Disconnected) => {
                    self.dav.rx = None;
                    self.dav.status = tl!("Calendar and contacts sync stopped unexpectedly; local edits retained").into();
                    self.status(self.dav.status.clone());
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if self.dav.rx.is_none() && self.services.dav.is_some() {
            let now = now_ms();
            let interval = if matches!(self.session.view.module, mailcraft_engine::Module::Calendar | mailcraft_engine::Module::People) {
                30_000.0
            } else {
                300_000.0
            };
            let due = self
                .session
                .mailbox
                .accounts
                .iter()
                .filter(|a| !a.dav_url.is_empty())
                .find(|a| {
                    let (last, rev) = self.dav.checked.get(&a.id).copied().unwrap_or((-300_000.0, 0));
                    let failures = self.dav.failures.get(&a.id).copied().unwrap_or(0);
                    let retry = retry_interval(interval, failures);
                    now - last >= retry
                        || (failures == 0
                            && now - last >= 2_000.0
                            && rev != self.session.rev()
                            && mailcraft_engine::dav::waiting(&self.session.mailbox, a.id))
                })
                .map(|a| a.id);
            if let Some(account) = due
                && let Err(error) = self.start_dav(&json!({"account":account,"quiet":true}))
            {
                self.dav.checked.insert(account, (now, self.session.rev()));
                self.dav.status = error;
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_imports_back_off_and_a_success_restores_the_normal_interval() {
        assert_eq!(retry_interval(30_000.0, 0), 30_000.0);
        assert_eq!(retry_interval(30_000.0, 1), 300_000.0);
        assert_eq!(retry_interval(30_000.0, 2), 600_000.0);
        assert_eq!(retry_interval(30_000.0, 3), 1_200_000.0);
        assert_eq!(retry_interval(30_000.0, u32::MAX), 1_800_000.0);
    }
    #[test]
    #[ignore = "manual light/dark preview requires a graphics adapter; writes synthetic screenshots to /tmp"]
    fn render_dav_progress_preview() {
        for (name, appearance) in [("light", crate::theme::Appearance::Light), ("dark", crate::theme::Appearance::Dark)] {
            let mailbox = mailcraft_engine::sample::mailbox(1_791_993_600, 0);
            let mut app = MailApp::new(mailcraft_engine::Session::new(mailbox), crate::Services::default());
            app.autosave = false;
            app.ui.theme = appearance;
            app.session.view.module = mailcraft_engine::Module::Calendar;
            let (_tx, rx) = std::sync::mpsc::channel();
            app.dav.rx = Some(rx);
            app.dav.status = "Calendar and contacts: 25 / 4400 items checked".into();
            let mut harness = egui_kittest::Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_ui_state(
                |ui, app: &mut MailApp| {
                    let ctx = ui.ctx().clone();
                    app.logic(&ctx);
                    app.ui(ui);
                },
                app,
            );
            for _ in 0..3 {
                harness.step();
            }
            harness.render().unwrap().save(format!("/tmp/mailcraft-dav-progress-{name}.png")).unwrap();
        }
    }
    #[test]
    fn progress_keeps_the_worker_active_and_final_reply_reuses_imported_items() {
        let mut mailbox = mailcraft_model::Mailbox::default();
        let account = mailbox.add_account(mailcraft_model::Account { dav_url: "https://dav.example.org/".into(), ..Default::default() });
        let mut app = MailApp::new(mailcraft_engine::Session::new(mailbox), crate::Services::default());
        let (tx, rx) = std::sync::mpsc::channel();
        app.dav.rx = Some(rx);
        app.dav.failures.insert(account, 3);
        let make = |progress| Download {
            account,
            url: "https://dav.example.org/".into(),
            progress,
            complete: progress.is_none(),
            resources: vec![mailcraft_model::dav::Resource {
                account,
                href: "/person.vcf".into(),
                etag: "1".into(),
                raw: "synthetic card".into(),
                contacts: vec![mailcraft_model::Contact { display_name: "Synthetic Person".into(), ..Default::default() }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let ctx = egui::Context::default();
        tx.send(make(Some((1, 25)))).unwrap();
        app.dav_tick(&ctx);
        assert!(app.dav.rx.is_some());
        assert_eq!(app.dav.failures.get(&account), Some(&3));
        assert_eq!(app.session.mailbox.contacts.len(), 1);
        assert!(app.dav.status.contains("1 / 25"));
        tx.send(make(None)).unwrap();
        app.dav_tick(&ctx);
        assert!(app.dav.rx.is_none());
        assert!(!app.dav.failures.contains_key(&account));
        assert_eq!(app.session.mailbox.contacts.len(), 1);
        assert!(!app.dav.status.contains("1 / 25"));
    }
}
