//! All write tests use this loopback DAV stand-in. No real accounts or servers.
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
const CAL: &str = "/dav/calendars/u/default/";
const BOOK: &str = "/dav/addressbooks/u/default/";
const ICS: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:event-1\r\nDTSTART:20261017T100000Z\r\nDTEND:20261017T110000Z\r\nSUMMARY:Appointment\r\nX-OPAQUE:keep-event\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
const VCF: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:contact-1\r\nFN:Original\r\nN:Original;;;;\r\nEMAIL:a@example.test\r\nPHOTO;ENCODING=b:abcdef\r\nX-OPAQUE:keep-contact\r\nEND:VCARD\r\n";
#[derive(Default)]
struct State {
    items: BTreeMap<String, (u64, String)>,
    writes: Vec<(String, String)>,
    refuse: bool,
    broken: bool,
    pending: bool,
    contacts_only: bool,
}
struct Fake {
    url: String,
    state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
fn prop(href: &str, properties: &str) -> String {
    format!(
        "<d:response><d:href>{}</d:href><d:propstat><d:prop>{properties}</d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>",
        quick_xml::escape::escape(href)
    )
}
fn multistatus(rows: &str) -> String {
    format!("<d:multistatus xmlns:d='DAV:' xmlns:c='{C}' xmlns:a='{A}'>{rows}</d:multistatus>")
}
impl Fake {
    fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let mut seed = State::default();
        seed.items.insert(format!("{CAL}event.ics"), (1, ICS.into()));
        seed.items.insert(format!("{BOOK}contact.vcf"), (1, VCF.into()));
        let state = Arc::new(Mutex::new(seed));
        let stop = Arc::new(AtomicBool::new(false));
        let shared = state.clone();
        let done = stop.clone();
        let worker = thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                };
                socket.set_nonblocking(false).unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
                let mut bytes = Vec::new();
                let mut block = [0u8; 4096];
                let mut end = None;
                while end.is_none() {
                    let n = socket.read(&mut block).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&block[..n]);
                    end = bytes.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
                    assert!(bytes.len() < 65536);
                }
                let end = end.unwrap();
                let head = String::from_utf8(bytes[..end].to_vec()).unwrap();
                let mut words = head.lines().next().unwrap().split_whitespace();
                let method = words.next().unwrap();
                let path = words.next().unwrap();
                let headers: BTreeMap<_, _> =
                    head.lines().skip(1).filter_map(|l| l.split_once(':')).map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string())).collect();
                assert_eq!(headers.get("authorization").map(String::as_str), Some("Basic dTpzZWNyZXQ="));
                let len: usize = headers.get("content-length").and_then(|s| s.parse().ok()).unwrap_or(0);
                while bytes.len() < end + len {
                    let n = socket.read(&mut block).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&block[..n]);
                }
                let body = String::from_utf8(bytes[end..end + len].to_vec()).unwrap();
                let mut s = shared.lock().unwrap();
                let mut status = 207;
                let mut answer = String::new();
                match method {
                    "PROPFIND" => {
                        let rows = if path == "/dav/" {
                            prop(path, "<d:current-user-principal><d:href>/dav/principals/u/</d:href></d:current-user-principal>")
                        } else if path == "/dav/principals/u/" && s.contacts_only {
                            prop(path, "<a:addressbook-home-set><d:href>/dav/calendars/u/</d:href></a:addressbook-home-set>")
                        } else if path == "/dav/principals/u/" {
                            prop(
                                path,
                                "<c:calendar-home-set><d:href>/dav/calendars/u/</d:href></c:calendar-home-set><a:addressbook-home-set><d:href>/dav/addressbooks/u/</d:href></a:addressbook-home-set>",
                            )
                        } else if path == "/dav/calendars/u/" || path == "/dav/addressbooks/u/" {
                            let (href, kind) = if s.contacts_only {
                                (CAL, "a:addressbook")
                            } else if path.contains("calendars") {
                                (CAL, "c:calendar")
                            } else {
                                (BOOK, "a:addressbook")
                            };
                            let mut rows = prop(
                                href,
                                &format!(
                                    "<d:displayname>Personal</d:displayname><d:resourcetype><d:collection/><{kind}/></d:resourcetype><d:current-user-privilege-set><d:privilege><d:write/></d:privilege></d:current-user-privilege-set>"
                                ),
                            );
                            if path.contains("calendars") {
                                rows.push_str(&prop("/dav/calendars/u/tasks/","<d:resourcetype><d:collection/><c:calendar/></d:resourcetype><c:supported-calendar-component-set><c:comp name='VTODO'/></c:supported-calendar-component-set>"));
                            }
                            rows
                        } else if path == CAL || path == BOOK {
                            let mut rows = prop(path, "<d:resourcetype><d:collection/></d:resourcetype>");
                            for (href, (tag, _)) in s.items.iter().filter(|(h, _)| h.starts_with(path)) {
                                rows.push_str(&prop(
                                    href,
                                    &if s.broken { String::new() } else { format!("<d:getetag>&quot;{tag}&quot;</d:getetag>") },
                                ));
                            }
                            rows
                        } else if let Some((tag, _)) = s.items.get(path) {
                            prop(path, &format!("<d:getetag>&quot;{tag}&quot;</d:getetag>"))
                        } else {
                            status = 404;
                            String::new()
                        };
                        answer = multistatus(&rows);
                    }
                    "GET" => {
                        if let Some((_, raw)) = s.items.get(path) {
                            status = 200;
                            answer = raw.clone();
                        } else {
                            status = 404;
                        }
                    }
                    "PUT" | "DELETE" => {
                        let matches = if let Some((tag, _)) = s.items.get(path) {
                            headers.get("if-match") == Some(&format!("\"{tag}\""))
                        } else {
                            headers.get("if-none-match").map(String::as_str) == Some("*")
                        };
                        if s.pending {
                            status = 202;
                        } else if s.refuse || !matches {
                            status = 412;
                        } else {
                            s.writes.push((method.into(), path.into()));
                            if method == "DELETE" {
                                s.items.remove(path);
                                status = 204;
                            } else {
                                let tag = s.items.get(path).map_or(1, |(t, _)| t + 1);
                                s.items.insert(path.into(), (tag, body));
                                status = 201;
                            }
                        }
                    }
                    _ => status = 405,
                }
                drop(s);
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\nContent-Type: application/xml\r\n\r\n{answer}",
                    answer.len()
                );
                socket.write_all(response.as_bytes()).unwrap();
            }
        });
        Self { url: format!("http://127.0.0.1:{port}/dav/"), state, stop, worker: Some(worker) }
    }
    fn mailbox(&self) -> Mailbox {
        let mut m = Mailbox::default();
        m.zone = "Europe/London".into();
        m.accounts.push(mailcraft_model::Account {
            id: 1,
            email: "u@example.test".into(),
            username: "u".into(),
            dav_url: self.url.clone(),
            ..Default::default()
        });
        m
    }
    fn sync(&self, m: &Mailbox, read_only: bool) -> Download {
        sync(Request { account: 1, password: "secret".into(), mailbox: m.clone(), read_only })
    }
}
impl Drop for Fake {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn adopt(m: &mut Mailbox, mut d: Download) {
    assert!(d.complete, "{:?}", d.errors);
    assert!(d.errors.is_empty(), "{:?}", d.errors);
    for c in &d.collections {
        if !c.contacts && !m.calendars.iter().any(|b| b.origin.id == c.href) {
            m.calendars.push(mailcraft_model::Calendar { id: 2, origin: origin(1, &c.href, ""), ..Default::default() });
        }
    }
    m.contacts.clear();
    m.events.clear();
    for r in &mut d.resources {
        for c in &mut r.contacts {
            if c.id == 0 {
                c.id = m.alloc_id();
            }
            m.contacts.insert(c.id, Arc::new(c.clone()));
        }
        for e in &mut r.events {
            e.calendar = 2;
            if e.id == 0 {
                e.id = m.alloc_id();
            }
            m.events.insert(e.id, Arc::new(e.clone()));
        }
    }
    m.dav_resources = d.resources;
    m.dav_collections = d.collections;
}
#[test]
fn separate_servers_with_identical_paths_sync_to_the_correct_origin_and_fail_atomically() {
    let calendar = Fake::new();
    let contacts = Fake::new();
    let path = format!("{CAL}same");
    {
        let mut s = calendar.state.lock().unwrap();
        s.items.clear();
        s.items.insert(path.clone(), (1, ICS.into()));
        let mut s = contacts.state.lock().unwrap();
        s.contacts_only = true;
        s.items.clear();
        s.items.insert(path.clone(), (1, VCF.into()));
    }
    let mut m = calendar.mailbox();
    m.accounts[0].carddav_url = contacts.url.clone();
    check_account(&m.accounts[0], "secret").unwrap();
    let d = calendar.sync(&m, true);
    assert!(d.complete && d.errors.is_empty(), "{:?}", d.errors);
    assert_eq!(d.resources.len(), 2);
    assert_ne!(d.resources[0].href, d.resources[1].href);
    assert!(d.resources.iter().all(|r| r.href.starts_with("http://127.0.0.1:") && r.href.ends_with(&path)));
    adopt(&mut m, d);
    let contact = *m.contacts.keys().next().unwrap();
    let event = *m.events.keys().next().unwrap();
    m.contact_mut(contact).unwrap().display_name = "Updated person".into();
    m.event_mut(event).unwrap().title = "Updated event".into();
    let d = calendar.sync(&m, false);
    assert!(d.complete && d.errors.is_empty(), "{:?}", d.errors);
    assert_eq!(d.uploaded, 2);
    adopt(&mut m, d);
    assert!(calendar.state.lock().unwrap().items[&path].1.contains("Updated event"));
    assert!(contacts.state.lock().unwrap().items[&path].1.contains("Updated person"));
    m.event_mut(event).unwrap().title = "Pending event".into();
    contacts.state.lock().unwrap().broken = true;
    let d = calendar.sync(&m, false);
    assert!(!d.complete && !d.errors.is_empty());
    assert_eq!(calendar.state.lock().unwrap().writes.len(), 1);
    assert_eq!(contacts.state.lock().unwrap().writes.len(), 1);
}
#[test]
fn setup_checks_calendar_and_contacts_without_writing() {
    let fake = Fake::new();
    let mailbox = fake.mailbox();
    check_account(&mailbox.accounts[0], "secret").unwrap();
    assert!(fake.state.lock().unwrap().writes.is_empty());
    let mut missing = mailbox.accounts[0].clone();
    missing.dav_url = missing.dav_url.replace("/dav/", "/missing/");
    assert!(check_account(&missing, "secret").is_err());
    assert!(fake.state.lock().unwrap().writes.is_empty());
}
#[test]
fn downloads_edits_creates_deletes_and_keeps_opaque_fields() {
    let fake = Fake::new();
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    assert!(fake.state.lock().unwrap().writes.is_empty());
    adopt(&mut m, d);
    let contact = *m.contacts.keys().next().unwrap();
    let event = *m.events.keys().next().unwrap();
    m.contact_mut(contact).unwrap().display_name = "Edited contact".into();
    m.event_mut(event).unwrap().title = "Edited appointment".into();
    let d = fake.sync(&m, false);
    assert_eq!(d.uploaded, 2);
    adopt(&mut m, d);
    {
        let s = fake.state.lock().unwrap();
        assert!(s.items[&format!("{BOOK}contact.vcf")].1.contains("PHOTO;ENCODING=b:abcdef"));
        assert!(s.items[&format!("{CAL}event.ics")].1.contains("X-OPAQUE:keep-event"));
    }
    let new = m.add_contact(Contact {
        display_name: "Created".into(),
        uid: "stable-created-uid".into(),
        origin: origin(1, BOOK, ""),
        ..Default::default()
    });
    let d = fake.sync(&m, false);
    assert_eq!(d.uploaded, 1);
    adopt(&mut m, d);
    assert!(m.contact(new).is_some());
    m.remove_contact(contact).unwrap();
    m.remove_event(event).unwrap();
    let d = fake.sync(&m, false);
    assert_eq!(d.uploaded, 2);
    adopt(&mut m, d);
    assert_eq!(fake.state.lock().unwrap().items.len(), 1);
    assert_eq!(m.contacts.len(), 1);
    assert!(m.events.is_empty());
}
#[test]
fn conflict_and_partial_inventory_never_overwrite_or_prune() {
    let fake = Fake::new();
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    adopt(&mut m, d);
    let id = *m.contacts.keys().next().unwrap();
    m.contact_mut(id).unwrap().display_name = "Local edit".into();
    fake.state.lock().unwrap().refuse = true;
    let d = fake.sync(&m, false);
    assert!(d.complete);
    assert!(d.errors.iter().any(|e| e.contains("conflict")));
    assert!(fake.state.lock().unwrap().writes.is_empty());
    assert_eq!(d.resources.len(), 2);
    fake.state.lock().unwrap().broken = true;
    let d = fake.sync(&m, false);
    assert!(!d.complete);
    assert_eq!(d.resources, m.dav_resources);
    assert!(fake.state.lock().unwrap().writes.is_empty());
}
#[test]
fn meeting_changes_never_trigger_scheduling() {
    let fake = Fake::new();
    fake.state.lock().unwrap().items.get_mut(&format!("{CAL}event.ics")).unwrap().1 =
        ICS.replace("SUMMARY:", "ATTENDEE:mailto:guest@example.test\r\nSUMMARY:");
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    adopt(&mut m, d);
    let id = *m.events.keys().next().unwrap();
    m.event_mut(id).unwrap().title = "Changed meeting".into();
    let d = fake.sync(&m, false);
    assert!(d.errors.iter().any(|e| e.contains("invitations")));
    assert!(fake.state.lock().unwrap().writes.is_empty());
}

#[test]
fn restart_recovers_a_create_with_a_lost_acknowledgment() {
    let fake = Fake::new();
    let mut m = fake.mailbox();
    let uid = "pending-created";
    let id =
        m.add_contact(Contact { uid: uid.into(), display_name: "Created before crash".into(), origin: origin(1, BOOK, ""), ..Default::default() });
    let raw = mailcraft_vcard::write(&[m.contact(id).unwrap()]);
    fake.state.lock().unwrap().items.insert(format!("{BOOK}{uid}.vcf"), (1, raw));
    let d = fake.sync(&m, false);
    assert!(d.complete, "{:?}", d.errors);
    assert!(d.errors.is_empty(), "{:?}", d.errors);
    assert!(d.resources.iter().flat_map(|r| &r.contacts).any(|c| c.uid == uid && c.id == id));
    assert!(fake.state.lock().unwrap().writes.is_empty());
}

#[test]
fn restart_recovers_lost_update_and_delete_acknowledgments() {
    let fake = Fake::new();
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    adopt(&mut m, d);
    let contact = *m.contacts.keys().next().unwrap();
    m.contact_mut(contact).unwrap().display_name = "Update already applied".into();
    let raw = mailcraft_vcard::write(&[m.contact(contact).unwrap()]);
    fake.state.lock().unwrap().items.insert(format!("{BOOK}contact.vcf"), (2, raw));
    let d = fake.sync(&m, false);
    assert!(d.errors.is_empty(), "{:?}", d.errors);
    adopt(&mut m, d);
    assert!(fake.state.lock().unwrap().writes.is_empty());
    m.remove_contact(contact).unwrap();
    fake.state.lock().unwrap().items.remove(&format!("{BOOK}contact.vcf"));
    let d = fake.sync(&m, false);
    assert!(d.errors.is_empty(), "{:?}", d.errors);
    assert!(!d.resources.iter().any(|r| r.href == format!("{BOOK}contact.vcf")));
    assert!(fake.state.lock().unwrap().writes.is_empty());
}

#[test]
fn an_async_acceptance_never_discards_pending_edits() {
    let fake = Fake::new();
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    assert_eq!(d.collections.len(), 2);
    adopt(&mut m, d);
    let contact = *m.contacts.keys().next().unwrap();
    m.contact_mut(contact).unwrap().display_name = "Still pending".into();
    fake.state.lock().unwrap().pending = true;
    let d = fake.sync(&m, false);
    assert!(d.errors.iter().any(|e| e.contains("pending")));
    assert_eq!(d.resources, m.dav_resources);
    assert!(fake.state.lock().unwrap().writes.is_empty());
}

#[test]
fn new_personal_appointments_use_a_stable_uid_and_server_calendar() {
    let fake = Fake::new();
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    adopt(&mut m, d);
    let mut e = (**m.events.values().next().unwrap()).clone();
    e.id = 0;
    e.uid = "new-personal-appointment".into();
    e.title = "Created appointment".into();
    e.origin = Origin { account: 1, ..Default::default() };
    let id = m.add_event(e).unwrap();
    let d = fake.sync(&m, false);
    assert_eq!(d.uploaded, 1);
    assert!(d.errors.is_empty(), "{:?}", d.errors);
    adopt(&mut m, d);
    assert_eq!(m.event(id).unwrap().uid, "new-personal-appointment");
    assert_eq!(m.event(id).unwrap().calendar, 2);
    let s = fake.state.lock().unwrap();
    assert!(s.items.contains_key(&format!("{CAL}new-personal-appointment.ics")));
    assert_eq!(s.writes.len(), 1);
}

#[test]
fn an_unreadable_new_item_is_skipped_without_stalling_the_account() {
    let fake = Fake::new();
    // A journal entry: kept on the server, never shown here, and no reason to stop the rest.
    let journal = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VJOURNAL\r\nUID:journal-1\r\nDTSTART:20261017T100000Z\r\nSUMMARY:Notes\r\nEND:VJOURNAL\r\nEND:VCALENDAR\r\n";
    fake.state.lock().unwrap().items.insert(format!("{CAL}journal.ics"), (1, journal.into()));
    let mut m = fake.mailbox();
    let d = fake.sync(&m, true);
    assert!(d.complete, "{:?}", d.errors);
    assert_eq!(d.collections.len(), 2);
    assert!(d.errors.iter().any(|e| e.contains("left on the server")), "{:?}", d.errors);
    assert_eq!(d.resources.len(), 2);
    assert!(!d.resources.iter().any(|r| r.href.ends_with("journal.ics")));
    // (`adopt` takes a run with no errors: this one's only error is the skipped item.)
    let Download { collections, resources, .. } = d;
    adopt(&mut m, Download { complete: true, collections, resources, ..Default::default() });
    // Writes still go out, and the server's journal entry is untouched.
    let contact = *m.contacts.keys().next().unwrap();
    m.contact_mut(contact).unwrap().display_name = "Edited past the journal".into();
    let d = fake.sync(&m, false);
    assert!(d.complete, "{:?}", d.errors);
    assert_eq!(d.uploaded, 1);
    let s = fake.state.lock().unwrap();
    assert_eq!(s.items[&format!("{CAL}journal.ics")].1, journal);
    assert_eq!(s.writes.len(), 1);
}
