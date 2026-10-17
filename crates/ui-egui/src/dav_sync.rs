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
    pub status: String,
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
                    let account = download.account;
                    let resources = download.resources.len();
                    let uploaded = download.uploaded;
                    let mut errors = download.errors.clone();
                    let complete = download.complete;
                    self.dav.rx = None;
                    errors.extend(mailcraft_engine::dav::apply(&mut self.session, download));
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
                    now - last >= interval
                        || (now - last >= 2_000.0 && rev != self.session.rev() && mailcraft_engine::dav::waiting(&self.session.mailbox, a.id))
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
