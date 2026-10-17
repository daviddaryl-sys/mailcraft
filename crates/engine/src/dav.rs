//! Merge DAV checkpoints without overwriting edits made while the network job was running.
use crate::Session;
use mailcraft_model::dav::{Download, Resource};
use mailcraft_model::{Calendar, Contact, Event, Origin, Rgb};
use std::sync::Arc;

fn same_contact(a: &Contact, b: &Contact) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.origin = Origin::default();
    b.origin = Origin::default();
    a == b
}
fn same_event(a: &Event, b: &Event) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.origin = Origin::default();
    b.origin = Origin::default();
    a == b
}
fn before_contact(resources: &[Resource], id: u64) -> Option<&Contact> {
    resources.iter().flat_map(|r| &r.contacts).find(|c| c.id == id && id != 0)
}
fn before_event(resources: &[Resource], id: u64) -> Option<&Event> {
    resources.iter().flat_map(|r| &r.events).find(|e| e.id == id && id != 0)
}
/// Returns true only when persisted state changed. Discard replies for edited/removed accounts.
pub fn apply(session: &mut Session, mut download: Download) -> Vec<String> {
    if session.mailbox.account(download.account).is_none_or(|a| {
        a.dav_url != download.url || a.carddav_url != download.carddav_url || download.login.as_ref().is_some_and(|login| login != a.dav_login())
    }) {
        return Vec::new();
    }
    let mut conflicts = Vec::new();
    let saved = session.mailbox.clone();
    let mailbox = &mut session.mailbox;
    for collection in &download.collections {
        if collection.contacts {
            continue;
        }
        if let Some(c) = mailbox.calendars.iter_mut().find(|c| c.origin.account == download.account && c.origin.id == collection.href) {
            c.name = collection.name.clone();
            c.read_only = collection.read_only;
        } else {
            let id = mailbox.alloc_id();
            mailbox.calendars.push(Calendar {
                id,
                name: collection.name.clone(),
                color: Rgb(54, 130, 180),
                visible: true,
                origin: Origin { account: download.account, id: collection.href.clone(), ..Default::default() },
                read_only: collection.read_only,
                group: mailbox.account(download.account).map_or("DAV".into(), |a| a.name.clone()),
                online_providers: Some(Vec::new()),
                ..Default::default()
            });
        }
    }
    // Missing resources are only deletions after a complete unfiltered inventory, or verified DELETE.
    let gone: Vec<_> = download
        .before
        .iter()
        .filter(|b| !download.resources.iter().any(|n| n.href == b.href) && (download.complete || download.removed.contains(&b.href)))
        .collect();
    let mut retained: Vec<_> = download
        .before
        .iter()
        .filter(|b| !download.complete && !download.removed.contains(&b.href) && !download.resources.iter().any(|n| n.href == b.href))
        .cloned()
        .collect();
    for before in gone {
        let changed = before.contacts.iter().any(|b| mailbox.contact(b.id).is_some_and(|c| !same_contact(c, b)))
            || before.events.iter().any(|b| mailbox.event(b.id).is_some_and(|e| !same_event(e, b)));
        if changed {
            conflicts.push("DAV conflict: an item deleted on the server was edited during sync; local copy retained".into());
            retained.push(before.clone());
            continue;
        }
        for c in &before.contacts {
            let _ = mailbox.remove_contact(c.id);
        }
        for e in &before.events {
            let _ = mailbox.remove_event(e.id);
        }
    }
    for resource in &mut download.resources {
        if let Some(base) = download.before.iter().find(|b| b.href == resource.href) {
            // A refused write or unchanged server resource never rolls back local edits.
            if base.etag == resource.etag && base.raw == resource.raw {
                continue;
            }
            let changed_during = resource.contacts.iter().any(|c| {
                before_contact(&download.snapshots, c.id)
                    .is_some_and(|asked| mailbox.contact(c.id).is_some_and(|current| !same_contact(current, asked)))
            }) || resource.events.iter().any(|e| {
                before_event(&download.snapshots, e.id).is_some_and(|asked| mailbox.event(e.id).is_some_and(|current| !same_event(current, asked)))
            });
            let removed_events: Vec<_> = base.events.iter().filter(|old| !resource.events.iter().any(|now| now.id == old.id)).collect();
            let removed_contacts: Vec<_> = base.contacts.iter().filter(|old| !resource.contacts.iter().any(|now| now.id == old.id)).collect();
            let removed_changed = removed_events.iter().any(|old| mailbox.event(old.id).is_some_and(|now| !same_event(now, old)))
                || removed_contacts.iter().any(|old| mailbox.contact(old.id).is_some_and(|now| !same_contact(now, old)));
            if (changed_during || removed_changed) && !download.written.contains(&resource.href) {
                conflicts.push("DAV conflict: local and server copies changed during sync; local edit retained".into());
                *resource = base.clone();
                continue;
            }
            for old in removed_events {
                let _ = mailbox.remove_event(old.id);
            }
            for old in removed_contacts {
                let _ = mailbox.remove_contact(old.id);
            }
        }

        for c in &mut resource.contacts {
            let prior = before_contact(&download.snapshots, c.id);
            let current = mailbox.contact(c.id).cloned();
            match (current, prior) {
                (Some(current), Some(prior)) => {
                    if same_contact(&current, prior) {
                        mailbox.contacts.insert(c.id, Arc::new(c.clone()));
                    } else if let Ok(local) = mailbox.contact_mut(c.id) {
                        local.origin = c.origin.clone();
                    }
                }
                (None, Some(_)) => {} // Deleted locally while the download ran: next job sends the delete.
                (Some(_), None) => {} // An unrelated id must never be replaced.
                (None, None) => {
                    c.id = mailbox.add_contact(c.clone());
                }
            }
        }
        let calendar = mailbox.calendars.iter().find(|c| c.origin.account == download.account && c.origin.id == resource.collection).map(|c| c.id);
        for e in &mut resource.events {
            if let Some(calendar) = calendar {
                e.calendar = calendar;
            }
            let prior = before_event(&download.snapshots, e.id);
            let current = mailbox.event(e.id).cloned();
            match (current, prior) {
                (Some(current), Some(prior)) => {
                    if same_event(&current, prior) {
                        mailbox.events.insert(e.id, Arc::new(e.clone()));
                    } else if let Ok(local) = mailbox.event_mut(e.id) {
                        local.origin = e.origin.clone();
                    }
                }
                (None, Some(_)) => {}
                (Some(_), None) => {}
                (None, None) => {
                    if let Ok(id) = mailbox.add_event(e.clone()) {
                        e.id = id;
                    }
                }
            }
        }
    }
    mailbox.dav_resources.retain(|r| r.account != download.account);
    mailbox.dav_resources.extend(download.resources);
    mailbox.dav_resources.extend(retained);
    if download.complete {
        mailbox.dav_collections.retain(|c| c.account != download.account);
        mailbox.dav_collections.extend(download.collections);
    }
    if *mailbox != saved {
        session.settle_dav_history(download.account, &saved);
        session.mailbox_changed();
    }
    conflicts
}

/// Whether there are local creations/edits/deletions to send for this DAV account.
pub fn waiting(mailbox: &mailcraft_model::Mailbox, account: u64) -> bool {
    if mailbox.dav_resources.iter().filter(|r| r.account == account).any(|r| {
        r.contacts.iter().any(|b| mailbox.contact(b.id).is_none_or(|c| !same_contact(c, b)))
            || r.events.iter().any(|b| mailbox.event(b.id).is_none_or(|e| !same_event(e, b)))
    }) {
        return true;
    }
    mailbox.contacts.values().any(|c| {
        c.origin.account == account
            && mailbox.dav_collections.iter().any(|b| b.contacts && !b.read_only && b.href == c.origin.id && b.account == account)
    }) || mailbox.events.values().any(|e| {
        mailbox.calendar(e.calendar).is_some_and(|c| c.origin.account == account)
            && !mailbox.dav_resources.iter().any(|r| r.account == account && r.events.iter().any(|b| b.id == e.id))
    })
}

/// Has an undo step changed this DAV account? Once synchronized, such steps are settled;
/// unrelated mail undo steps are retained and rebased onto the downloaded DAV state.
pub(crate) fn different(a: &mailcraft_model::Mailbox, b: &mailcraft_model::Mailbox, account: u64) -> bool {
    a.account(account).map(|a| (&a.dav_url, &a.carddav_url, a.dav_login())) != b.account(account).map(|a| (&a.dav_url, &a.carddav_url, a.dav_login()))
        || !a.contacts.values().filter(|c| c.origin.account == account).eq(b.contacts.values().filter(|c| c.origin.account == account))
        || !a.events.values().filter(|e| e.origin.account == account).eq(b.events.values().filter(|e| e.origin.account == account))
        || !a.calendars.iter().filter(|c| c.origin.account == account).eq(b.calendars.iter().filter(|c| c.origin.account == account))
        || !a.dav_resources.iter().filter(|r| r.account == account).eq(b.dav_resources.iter().filter(|r| r.account == account))
}
pub(crate) fn rebase_snapshot(snapshot: &mut mailcraft_model::Mailbox, latest: &mailcraft_model::Mailbox, account: u64) {
    if let (Some(old), Some(now)) = (snapshot.accounts.iter_mut().find(|a| a.id == account), latest.account(account)) {
        old.dav_url = now.dav_url.clone();
        old.carddav_url = now.carddav_url.clone();
        old.dav_username = now.dav_username.clone();
    } else {
        return;
    }
    snapshot.contacts.retain(|_, c| c.origin.account != account);
    for c in latest.contacts.values().filter(|c| c.origin.account == account) {
        snapshot.add_contact((**c).clone());
    }
    snapshot.calendars.retain(|c| c.origin.account != account);
    for c in latest.calendars.iter().filter(|c| c.origin.account == account) {
        snapshot.reserve_id(c.id);
        snapshot.calendars.push(c.clone());
    }
    snapshot.events.retain(|_, e| e.origin.account != account);
    for e in latest.events.values().filter(|e| e.origin.account == account) {
        let _ = snapshot.add_event((**e).clone());
    }
    snapshot.dav_resources.retain(|r| r.account != account);
    snapshot.dav_resources.extend(latest.dav_resources.iter().filter(|r| r.account == account).cloned());
    snapshot.dav_collections.retain(|c| c.account != account);
    snapshot.dav_collections.extend(latest.dav_collections.iter().filter(|c| c.account == account).cloned());
}

#[cfg(test)]
#[path = "dav_tests.rs"]
mod tests;
