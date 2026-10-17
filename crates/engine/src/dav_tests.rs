use super::*;
use mailcraft_model::{
    Account, Mailbox,
    dav::{Collection, Download, Resource},
};
use serde_json::json;
const URL: &str = "https://dav.example.test/dav/";
const BOOK: &str = "/dav/addressbooks/u/default/";
fn session() -> Session {
    let mut mailbox = Mailbox::default();
    mailbox.add_account(Account { email: "u@example.test".into(), dav_url: URL.into(), ..Default::default() });
    Session::new(mailbox)
}
fn initial(s: &Session) -> Download {
    let account = s.mailbox.accounts[0].id;
    Download {
        account,
        url: URL.into(),
        collections: vec![Collection { account, href: BOOK.into(), contacts: true, ..Default::default() }],
        resources: vec![Resource {
            account,
            href: format!("{BOOK}a.vcf"),
            collection: BOOK.into(),
            etag: "\"1\"".into(),
            raw: "vcard1".into(),
            contacts: vec![Contact {
                display_name: "Original".into(),
                uid: "a".into(),
                origin: Origin { account, id: format!("{BOOK}a.vcf"), change_key: "\"1\"".into(), ..Default::default() },
                ..Default::default()
            }],
            ..Default::default()
        }],
        complete: true,
        ..Default::default()
    }
}
fn update(s: &Session, name: &str, written: bool) -> Download {
    let before = s.mailbox.dav_resources.clone();
    let mut resources = before.clone();
    resources[0].contacts[0].display_name = name.into();
    resources[0].contacts[0].origin.change_key = "\"2\"".into();
    resources[0].etag = "\"2\"".into();
    resources[0].raw = "vcard2".into();
    let snapshots = before
        .iter()
        .map(|b| {
            let mut snapshot = b.clone();
            snapshot.contacts = b.contacts.iter().filter_map(|c| s.mailbox.contact(c.id).cloned()).collect();
            snapshot
        })
        .collect();
    Download {
        account: s.mailbox.accounts[0].id,
        url: URL.into(),
        written: if written { vec![resources[0].href.clone()] } else { vec![] },
        before,
        snapshots,
        resources,
        collections: s.mailbox.dav_collections.clone(),
        complete: true,
        ..Default::default()
    }
}
fn seeded() -> Session {
    let mut s = session();
    let d = initial(&s);
    assert!(apply(&mut s, d).is_empty());
    s
}
#[test]
fn merge_does_not_duplicate_contacts_and_respects_remote_updates() {
    let mut s = seeded();
    assert_eq!(s.mailbox.contacts.len(), 1);
    let d = update(&s, "Server edit", false);
    apply(&mut s, d);
    assert_eq!(s.mailbox.contacts.len(), 1);
    assert_eq!(s.mailbox.contacts.values().next().unwrap().display_name, "Server edit");
}
#[test]
fn concurrent_edits_keep_the_old_etag_and_require_conflict_resolution() {
    let mut s = seeded();
    let d = update(&s, "Server edit", false);
    let id = *s.mailbox.contacts.keys().next().unwrap();
    s.mailbox.contact_mut(id).unwrap().display_name = "Concurrent local edit".into();
    assert!(!apply(&mut s, d).is_empty());
    assert_eq!(s.mailbox.contact(id).unwrap().display_name, "Concurrent local edit");
    assert_eq!(s.mailbox.dav_resources[0].etag, "\"1\"");
    let account = s.mailbox.accounts[0].id;
    let href = s.mailbox.dav_resources[0].href.clone();
    s.run("dav.useServer", &json!({"account":account,"href":href})).unwrap();
    assert_eq!(s.mailbox.contacts.len(), 1);
    assert!(s.mailbox.contacts.values().next().unwrap().origin.is_local());
    assert!(s.mailbox.dav_resources.is_empty());
}
#[test]
fn a_write_acknowledgment_keeps_newer_local_edits_queued() {
    let mut s = seeded();
    let id = *s.mailbox.contacts.keys().next().unwrap();
    s.mailbox.contact_mut(id).unwrap().display_name = "Edit sent".into();
    let d = update(&s, "Edit sent", true);
    s.mailbox.contact_mut(id).unwrap().display_name = "Newer edit".into();
    assert!(apply(&mut s, d).is_empty());
    assert_eq!(s.mailbox.contact(id).unwrap().display_name, "Newer edit");
    assert_eq!(s.mailbox.dav_resources[0].etag, "\"2\"");
    assert!(waiting(&s.mailbox, s.mailbox.accounts[0].id));
}
#[test]
fn a_refused_upload_never_undoes_local_changes() {
    let mut s = seeded();
    let id = *s.mailbox.contacts.keys().next().unwrap();
    s.mailbox.contact_mut(id).unwrap().display_name = "Local unsent edit".into();
    let mut d = update(&s, "Ignored", false);
    d.resources = d.before.clone();
    apply(&mut s, d);
    assert_eq!(s.mailbox.contact(id).unwrap().display_name, "Local unsent edit");
}
#[test]
fn incomplete_download_and_inflight_delete_are_safe() {
    let mut s = seeded();
    let id = *s.mailbox.contacts.keys().next().unwrap();
    let mut d = update(&s, "Server edit", false);
    s.mailbox.remove_contact(id).unwrap();
    assert!(apply(&mut s, d).is_empty());
    assert!(s.mailbox.contacts.is_empty());
    assert!(!s.mailbox.dav_resources.is_empty());
    d = update(&s, "Next", false);
    d.resources.clear();
    d.complete = false;
    apply(&mut s, d);
    assert_eq!(s.mailbox.dav_resources.len(), 1); // An incomplete result retains the checkpoint.
}
#[test]
fn commands_assign_new_contacts_and_events_to_dav_without_graph_queue() {
    let mut s = seeded();
    let account = s.mailbox.accounts[0].id;
    let added = s.run("people.new", &json!({"first":"Created","account":account})).unwrap();
    let id = added["id"].as_u64().unwrap();
    let c = s.mailbox.contact(id).unwrap();
    assert_eq!(c.origin.account, account);
    assert_eq!(c.origin.id, BOOK);
    assert!(!c.uid.is_empty());
    s.mailbox.calendars.push(Calendar {
        id: 900,
        name: "DAV".into(),
        origin: Origin { account, id: "/dav/calendars/u/default/".into(), ..Default::default() },
        ..Default::default()
    });
    let added = s.run("calendar.new", &json!({"calendar":900,"title":"Personal","start":"2026-10-17T10:00","end":"2026-10-17T11:00"})).unwrap();
    let id = added["id"].as_u64().unwrap();
    let e = s.mailbox.event(id).unwrap();
    assert_eq!(e.origin.account, account);
    assert!(!e.uid.is_empty());
    assert!(s.mailbox.event_changes.is_empty());
    assert!(s.run("calendar.edit", &json!({"id":id,"attendees":"guest@example.test","send":true})).is_err());
}
#[test]
fn disconnect_preserves_local_items_and_stale_results_are_ignored() {
    let mut s = seeded();
    let d = update(&s, "Stale", false);
    let account = s.mailbox.accounts[0].id;
    s.run("dav.disconnect", &json!({"account":account})).unwrap();
    assert!(s.mailbox.contacts.values().all(|c| c.origin.is_local()));
    assert!(apply(&mut s, d).is_empty());
    assert_eq!(s.mailbox.contacts.values().next().unwrap().display_name, "Original");
}

#[test]
fn unrelated_undo_cannot_delete_downloaded_dav_items() {
    let mut s = session();
    let account = s.mailbox.accounts[0].id;
    s.run("account.edit", &json!({"id":account,"displayName":"Mail name"})).unwrap();
    let d = initial(&s);
    apply(&mut s, d);
    assert!(s.can_undo());
    s.undo();
    assert_eq!(s.mailbox.accounts[0].display_name, "");
    assert_eq!(s.mailbox.contacts.len(), 1);
    assert_eq!(s.mailbox.dav_resources.len(), 1);
    assert!(!waiting(&s.mailbox, account));
}
#[test]
fn synchronized_dav_edits_settle_without_discarding_mail_undo() {
    let mut s = seeded();
    let account = s.mailbox.accounts[0].id;
    let id = *s.mailbox.contacts.keys().next().unwrap();
    s.run("account.edit", &json!({"id":account,"displayName":"Mail name"})).unwrap();
    s.run("people.edit", &json!({"id":id,"displayName":"Contact edit"})).unwrap();
    let d = update(&s, "Contact edit", true);
    apply(&mut s, d);
    assert!(s.can_undo());
    s.undo();
    assert_eq!(s.mailbox.accounts[0].display_name, "");
    assert_eq!(s.mailbox.contact(id).unwrap().display_name, "Contact edit");
    assert!(!waiting(&s.mailbox, account));
}

#[test]
fn removed_recurrence_components_do_not_become_new_uploads() {
    let mut s = seeded();
    let account = s.mailbox.accounts[0].id;
    let href = "/dav/calendars/u/default/a.ics";
    let collection = "/dav/calendars/u/default/";
    s.mailbox.calendars.push(Calendar {
        id: 900,
        name: "DAV".into(),
        origin: Origin { account, id: collection.into(), ..Default::default() },
        ..Default::default()
    });
    s.mailbox.reserve_id(900);
    let mut events = Vec::new();
    for uid in ["series", "series#20261018"] {
        let e = Event {
            calendar: 900,
            uid: uid.into(),
            origin: Origin { account, id: href.into(), change_key: "\"1\"".into(), ..Default::default() },
            ..Default::default()
        };
        let id = s.mailbox.add_event(e).unwrap();
        events.push(s.mailbox.event(id).unwrap().clone());
    }
    let removed = events[1].id;
    s.mailbox.dav_resources.push(Resource {
        account,
        collection: collection.into(),
        href: href.into(),
        etag: "\"1\"".into(),
        raw: "old series".into(),
        events,
        ..Default::default()
    });
    let before = s.mailbox.dav_resources.clone();
    let mut resources = before.clone();
    let r = resources.iter_mut().find(|r| r.href == href).unwrap();
    r.events.pop();
    r.raw = "updated series".into();
    r.etag = "\"2\"".into();
    let d = Download {
        account,
        url: URL.into(),
        snapshots: before.clone(),
        before,
        resources,
        collections: s.mailbox.dav_collections.clone(),
        complete: true,
        ..Default::default()
    };
    assert!(apply(&mut s, d).is_empty());
    assert!(s.mailbox.event(removed).is_none());
    assert!(!waiting(&s.mailbox, account));
}

#[test]
fn new_contacts_prefer_contacts_over_grommunio_recipient_cache() {
    let mut s = seeded();
    let account = s.mailbox.accounts[0].id;
    s.mailbox.dav_collections[0].name = "Contacts".into();
    let cache = "/dav/addressbooks/u/Recipient%20Cache/";
    s.mailbox.dav_collections.push(Collection { account, href: cache.into(), name: "Recipient Cache".into(), contacts: true, ..Default::default() });
    let added = s.run("people.new", &json!({"first":"Personal"})).unwrap();
    let id = added["id"].as_u64().unwrap();
    assert_eq!(s.mailbox.contact(id).unwrap().origin.id, BOOK);
    let added = s.run("people.new", &json!({"first":"Explicit cache","account":account,"addressBook":cache})).unwrap();
    let id = added["id"].as_u64().unwrap();
    assert_eq!(s.mailbox.contact(id).unwrap().origin.id, cache);
}
