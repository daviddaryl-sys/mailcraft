//! DAV connection status and explicit, lossless conflict recovery.
use super::{bad, fail};
use crate::{CmdResult, CommandSpec, Session, p};
use mailcraft_model::{Calendar, Origin, Rgb};
use serde_json::{Value, json};
pub fn specs() -> Vec<CommandSpec> {
    vec![
 CommandSpec::new("dav.status","Calendar and Contacts Sync Status","Send / Receive",status).pure().params(r#"{"account":number} — lists DAV collections, checkpoints and pending edits; never credentials or raw documents"#),
 CommandSpec::new("dav.useServer","Resolve DAV Conflict Using Server","Send / Receive",use_server).params(r#"{"account":number,"href":string} — preserves local edits as separate local copies, then downloads the server copy on the next sync; sends nothing"#),
 CommandSpec::new("dav.disconnect","Disconnect Calendar and Contacts","File › Info",disconnect).params(r#"{"account":number} — keeps downloaded items as local copies and disables DAV without changing the server"#),
]
}
fn status(s: &mut Session, v: &Value) -> CmdResult {
    let account = p::req_id(v, "account")?;
    let a = s.mailbox.account(account).ok_or_else(|| bad("No such account"))?;
    Ok(
        json!({"account":account,"url":a.dav_url,"pending":crate::dav::waiting(&s.mailbox,account),"collections":s.mailbox.dav_collections.iter().filter(|c|c.account==account).collect::<Vec<_>>(),"resources":s.mailbox.dav_resources.iter().filter(|r|r.account==account).map(|r|json!({"href":r.href,"contacts":r.contacts.iter().map(|c|c.id).collect::<Vec<_>>(),"events":r.events.iter().map(|e|e.id).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
    )
}
fn local_calendar(s: &mut Session) -> u64 {
    if let Some(c) = s.mailbox.calendars.iter().find(|c| c.origin.is_local()) {
        return c.id;
    }
    let id = s.mailbox.alloc_id();
    s.mailbox.calendars.push(Calendar { id, name: "Local conflict copies".into(), color: Rgb(160, 110, 60), visible: true, ..Default::default() });
    id
}
fn use_server(s: &mut Session, v: &Value) -> CmdResult {
    let account = p::req_id(v, "account")?;
    let href = p::req_str(v, "href")?;
    let before = s
        .mailbox
        .dav_resources
        .iter()
        .find(|r| r.account == account && r.href == href)
        .cloned()
        .ok_or_else(|| fail("No DAV checkpoint matches that resource"))?;
    let mut preserved = 0;
    for c in before.contacts {
        if let Some(mut current) = s.mailbox.contact(c.id).cloned() {
            if current != c {
                current.id = 0;
                current.origin = Origin::default();
                current.uid.clear();
                current.display_name = format!("{} (local conflict copy)", current.name());
                s.mailbox.add_contact(current);
                preserved += 1;
            }
            let _ = s.mailbox.remove_contact(c.id);
        }
    }
    for e in before.events {
        if let Some(mut current) = s.mailbox.event(e.id).cloned() {
            if current != e {
                current.id = 0;
                current.origin = Origin::default();
                current.uid.clear();
                current.title.push_str(" (local conflict copy)");
                current.calendar = local_calendar(s);
                s.mailbox.add_event(current)?;
                preserved += 1;
            }
            let _ = s.mailbox.remove_event(e.id);
        }
    }
    s.mailbox.dav_resources.retain(|r| !(r.account == account && r.href == href));
    Ok(json!({"preserved":preserved,"next":"ui.davSync"}))
}
fn disconnect(s: &mut Session, v: &Value) -> CmdResult {
    let account = p::req_id(v, "account")?;
    s.mailbox.account_mut(account)?.dav_url.clear();
    for contact in s.mailbox.contacts.values_mut() {
        let c = std::sync::Arc::make_mut(contact);
        if c.origin.account == account {
            c.origin = Origin::default();
        }
    }
    for event in s.mailbox.events.values_mut() {
        let e = std::sync::Arc::make_mut(event);
        if e.origin.account == account {
            e.origin = Origin::default();
        }
    }
    for calendar in &mut s.mailbox.calendars {
        if calendar.origin.account == account {
            calendar.origin = Origin::default();
            calendar.read_only = false;
        }
    }
    s.mailbox.dav_resources.retain(|r| r.account != account);
    s.mailbox.dav_collections.retain(|c| c.account != account);
    Ok(json!({"account":account,"disconnected":true}))
}
