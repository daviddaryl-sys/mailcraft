//! Offline migration verification on a disposable mailbox copy. Never connects to servers.
//! Usage: cargo run -p mailcraft-engine --example migration_check -- COPY.mailcraft
use mailcraft_engine::store;
use mailcraft_model::Mailbox;
use std::path::PathBuf;

fn snapshot(mailbox: &Mailbox) -> Result<serde_json::Value, String> {
    let mut v = serde_json::to_value(mailbox).map_err(|e| e.to_string())?;
    if let Some(fields) = v.as_object_mut() {
        fields.remove("version");
    }
    Ok(v)
}

fn main() -> Result<(), String> {
    let path = std::env::args_os().nth(1).map(PathBuf::from).ok_or("Pass a disposable mailbox copy".to_string())?;
    let mut before = store::open(&path)?;
    if before.unavailable != 0 || before.lost_records != 0 || before.lost_saves != 0 || before.unfinished != 0 {
        return Err("The input has unavailable content or incomplete records; no migration attempted".into());
    }
    store::load_all(&path, &mut before.mailbox)?;
    let expected = snapshot(&before.mailbox)?;
    store::save(&path, &before.mailbox)?;
    let mut after = store::open(&path)?;
    if after.unavailable != 0 || after.lost_records != 0 || after.lost_saves != 0 || after.unfinished != 0 {
        return Err("Migrated file reports missing content or incomplete records".into());
    }
    store::load_all(&path, &mut after.mailbox)?;
    if snapshot(&after.mailbox)? != expected {
        return Err("Mailbox state changed during migration; do not install".into());
    }
    println!(
        "Migration verified: {} accounts, {} messages, {} contacts, {} events, {} calendars, {} DAV checkpoints; all serialized account/item/content state preserved",
        after.mailbox.accounts.len(),
        after.mailbox.messages.len(),
        after.mailbox.contacts.len(),
        after.mailbox.events.len(),
        after.mailbox.calendars.len(),
        after.mailbox.dav_resources.len()
    );
    Ok(())
}
