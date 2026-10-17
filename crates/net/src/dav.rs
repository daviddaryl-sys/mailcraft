//! CalDAV/CardDAV discovery and optimistic, conditional two-way synchronization.
//! No redirects, cross-origin credentials, implicit scheduling, or unconditional overwrites.
mod document;
mod icloud;
mod xml;
use crate::http::{self, Url};
use base64::Engine as _;
use mailcraft_model::dav::{Collection, Download, Request, Resource};
use mailcraft_model::{Contact, Event, Mailbox, Origin};
use mailcraft_time::Zone;
use std::collections::{BTreeMap, HashSet};
use std::time::Duration;
use xml::{A, C, D};
const MAX_ITEMS: usize = 20_000;
const MAX_DOCUMENT: usize = 2 * 1024 * 1024;
const PROPS: &str = "<d:displayname/><d:resourcetype/><d:current-user-privilege-set/><d:current-user-principal/><c:calendar-home-set/><c:supported-calendar-component-set/><a:addressbook-home-set/><d:getetag/>";
struct Client {
    base: Url,
    auth: String,
    apple: Option<icloud::Service>,
    absolute: bool,
    #[cfg(test)]
    trusted_test_origins: Vec<Url>,
}
impl Client {
    fn new(url: &str, username: &str, password: &str) -> Result<Self, String> {
        if username.contains([':', '\r', '\n']) {
            return Err("Invalid DAV user name".into());
        }
        let base = Url::parse(url).map_err(|_| "DAV needs a valid HTTPS URL")?;
        safe_path(&base.path)?;
        let apple = icloud::Service::of(&base);
        Ok(Self {
            base,
            apple,
            absolute: apple.is_some(),
            auth: format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))),
            #[cfg(test)]
            trusted_test_origins: Vec::new(),
        })
    }
    fn allowed(&self, url: &Url) -> bool {
        if self.base.same_server(url) || self.apple.is_some_and(|apple| apple.allows(url)) {
            return true;
        }
        #[cfg(test)]
        if self.trusted_test_origins.iter().any(|origin| origin.same_server(url)) {
            return true;
        }
        false
    }
    fn href(&self, url: &Url) -> String {
        if self.absolute { full_url(url) } else { url.path.clone() }
    }
    fn url(&self, href: &str, relative: &str) -> Result<Url, String> {
        let mut context = self.base.clone();
        if relative.contains("://") {
            context = self.url_at(relative, &self.base)?;
        } else {
            context.path = relative.into();
        }
        self.url_at(href, &context)
    }
    fn url_at(&self, href: &str, context: &Url) -> Result<Url, String> {
        let text = if href.contains("://") {
            href.to_string()
        } else {
            if href.starts_with("//") {
                return Err("Cross-origin DAV href refused".into());
            }
            let mut url = context.clone();
            url.path = if href.starts_with('/') {
                href.into()
            } else {
                format!("{}{href}", context.path.rsplit_once('/').map_or("/".into(), |(p, _)| format!("{p}/")))
            };
            full_url(&url)
        };
        let url = Url::parse(&text).map_err(|_| "Invalid DAV href")?;
        if !self.allowed(&url) {
            return Err("Cross-origin DAV href refused".into());
        }
        safe_path(&url.path)?;
        Ok(url)
    }
    fn call(&self, url: &Url, method: &str, body: Option<&str>, extra: &[(&str, &str)]) -> Result<http::Reply, String> {
        self.call_at(url, method, body, extra).map(|(answer, _)| answer)
    }
    fn call_at(&self, first: &Url, method: &str, body: Option<&str>, extra: &[(&str, &str)]) -> Result<(http::Reply, Url), String> {
        let mut url = first.clone();
        let mut seen = HashSet::new();
        for hop in 0..=5 {
            if !self.allowed(&url) {
                return Err("Cross-origin DAV request refused".into());
            }
            safe_path(&url.path)?;
            if !seen.insert(full_url(&url)) {
                return Err("DAV redirect loop refused".into());
            }
            let mut headers = vec![("Authorization", self.auth.as_str())];
            headers.extend_from_slice(extra);
            let answer = http::request(
                &url,
                &http::Request { method, headers: &headers, body, patience: Duration::from_secs(40), max_body: http::MAX_LARGE_BODY },
            )
            .map_err(|_| "DAV connection failed (network, TLS, timeout or response limit)".to_string())?;
            let redirect = matches!(answer.status, 301 | 302 | 307 | 308) || (answer.status == 303 && method == "GET");
            let can_follow = self.apple.is_some();
            #[cfg(test)]
            let can_follow = can_follow || !self.trusted_test_origins.is_empty();
            if redirect && can_follow && matches!(method, "GET" | "PROPFIND") {
                if hop == 5 {
                    return Err("Too many DAV redirects".into());
                }
                url = self.url_at(answer.location.as_deref().ok_or("DAV redirect omitted its location")?, &url)?;
                continue;
            }
            if !(200..300).contains(&answer.status) {
                return Err(match answer.status {
                    401 if self.apple.is_some() => "iCloud sign-in refused; use a valid Apple app-specific password".into(),
                    401 => "DAV sign-in refused".into(),
                    403 => "DAV server refused permission".into(),
                    412 => "DAV conflict: the server copy changed; local edits are retained".into(),
                    code => format!("DAV HTTP {code}; no unconditional retry was made"),
                });
            }
            let final_status = match method {
                "GET" => answer.status == 200,
                "PUT" => matches!(answer.status, 200 | 201 | 204),
                "DELETE" => matches!(answer.status, 200 | 204),
                "PROPFIND" => answer.status == 207,
                _ => true,
            };
            if !final_status {
                return Err("DAV change is pending or the response is incomplete; local edits retained".into());
            }
            return Ok((answer, url));
        }
        Err("Too many DAV redirects".into())
    }
    fn prop(&self, url: &Url, depth: &str) -> Result<xml::Node, String> {
        self.prop_at(url, depth).map(|(node, _)| node)
    }
    fn prop_at(&self, url: &Url, depth: &str) -> Result<(xml::Node, Url), String> {
        let body =
            format!("<?xml version=\"1.0\"?><d:propfind xmlns:d=\"DAV:\" xmlns:c=\"{C}\" xmlns:a=\"{A}\"><d:prop>{PROPS}</d:prop></d:propfind>");
        let (reply, at) = self.call_at(url, "PROPFIND", Some(&body), &[("Depth", depth), ("Content-Type", "application/xml; charset=utf-8")])?;
        Ok((xml::parse(&reply.body)?, at))
    }
    fn discover(&self, account: u64) -> Result<Vec<Collection>, String> {
        let (root, context) = self.prop_at(&self.base, "0")?;
        let principal = root
            .children
            .iter()
            .filter(|n| n.is(D, "response"))
            .flat_map(xml::properties)
            .find(|n| n.is(D, "current-user-principal"))
            .and_then(|n| n.child(D, "href"))
            .map(|n| n.text.trim())
            .ok_or("DAV server did not identify a principal")?;
        let principal = self.url_at(principal, &context)?;
        let (root, principal) = self.prop_at(&principal, "0")?;
        let mut homes = Vec::new();
        for p in root.children.iter().filter(|n| n.is(D, "response")).flat_map(xml::properties) {
            if p.is(C, "calendar-home-set") || p.is(A, "addressbook-home-set") {
                for href in p.children.iter().filter(|n| n.is(D, "href")) {
                    let url = self.url_at(href.text.trim(), &principal)?;
                    if !homes.contains(&url) {
                        homes.push(url);
                    }
                }
            }
        }
        if homes.is_empty() {
            return Err("No CalDAV or CardDAV home was discovered".into());
        }
        let mut collections = Vec::new();
        for home in homes {
            let (root, home) = self.prop_at(&home, "1")?;
            for row in root.children.iter().filter(|n| n.is(D, "response")) {
                let props = xml::properties(row);
                let calendar = props.iter().any(|n| n.is(D, "resourcetype") && n.has(C, "calendar"));
                let events_allowed = props
                    .iter()
                    .find(|p| p.is(C, "supported-calendar-component-set"))
                    .is_none_or(|p| p.children.iter().any(|n| n.is(C, "comp") && n.attr_name == "VEVENT"));
                let calendar = calendar && events_allowed;
                let contacts = props.iter().any(|n| n.is(D, "resourcetype") && n.has(A, "addressbook"));
                if !calendar && !contacts {
                    continue;
                }
                let at = self.url_at(row.value(D, "href"), &home)?;
                if !at.same_server(&home) || !at.path.starts_with(&home.path) || !at.path.ends_with('/') {
                    return Err("DAV collection is outside its home".into());
                }
                let href = self.href(&at);
                let name = props.iter().find(|n| n.is(D, "displayname")).map_or("Calendar", |n| n.text.trim());
                let write = props
                    .iter()
                    .find(|n| n.is(D, "current-user-privilege-set"))
                    .is_some_and(|n| n.has(D, "all") || n.has(D, "write") || (n.has(D, "write-content") && n.has(D, "bind") && n.has(D, "unbind")));
                if collections.len() >= 256 {
                    return Err("Too many DAV collections".into());
                }
                collections.push(Collection { account, href, name: name.chars().take(120).collect(), contacts, read_only: !write });
            }
        }
        collections.sort_by(|a, b| a.href.cmp(&b.href));
        collections.dedup_by(|a, b| a.href == b.href);
        Ok(collections)
    }
    fn inventory(&self, collection: &Collection) -> Result<BTreeMap<String, String>, String> {
        let url = self.url(&collection.href, &self.base.path)?;
        let (root, url) = self.prop_at(&url, "1")?;
        let mut items = BTreeMap::new();
        let mut self_seen = false;
        for row in root.children.iter().filter(|n| n.is(D, "response")) {
            if row.value(D, "href").is_empty() {
                return Err("DAV inventory has no href".into());
            }
            let at = self.url_at(row.value(D, "href"), &url)?;
            if !at.same_server(&url) {
                return Err("DAV resource outside collection".into());
            }
            let path = self.href(&at);
            if at.path == url.path {
                if !xml::properties(row).iter().any(|p| p.is(D, "resourcetype") && p.has(D, "collection")) {
                    return Err("DAV collection listing is incomplete".into());
                }
                self_seen = true;
                continue;
            }
            if !at.path.starts_with(&url.path) || at.path.get(url.path.len()..).is_none_or(|tail| tail.contains('/')) {
                return Err("DAV resource outside collection".into());
            }
            let props = xml::properties(row);
            if props.iter().any(|n| n.is(D, "resourcetype") && n.has(D, "collection")) {
                continue;
            }
            let etag = props.iter().find(|n| n.is(D, "getetag")).map(|n| n.text.trim()).ok_or("Incomplete DAV inventory; no items were removed")?;
            strong_etag(etag)?;
            if items.insert(path, etag.to_string()).is_some() {
                return Err("Duplicate DAV resource".into());
            }
            if items.len() > MAX_ITEMS {
                return Err("DAV inventory exceeds item limit".into());
            }
        }
        if !self_seen {
            return Err("DAV collection listing is incomplete; existing items retained".into());
        }
        Ok(items)
    }
    fn fetch(&self, path: &str) -> Result<String, String> {
        let reply = self.call(&self.url(path, &self.base.path)?, "GET", None, &[])?;
        if reply.body.len() > MAX_DOCUMENT {
            return Err("DAV document exceeds size limit".into());
        }
        String::from_utf8(reply.body).map_err(|_| "DAV document is not UTF-8".into())
    }
    fn etag(&self, path: &str) -> Result<String, String> {
        let root = self.prop(&self.url(path, &self.base.path)?, "0")?;
        let etag = root
            .children
            .iter()
            .filter(|n| n.is(D, "response"))
            .flat_map(xml::properties)
            .find(|n| n.is(D, "getetag"))
            .map(|n| n.text.trim().to_string())
            .ok_or("DAV write could not be verified")?;
        strong_etag(&etag)?;
        Ok(etag)
    }
}
fn full_url(url: &Url) -> String {
    let host = if url.host.contains(':') { format!("[{}]", url.host) } else { url.host.to_ascii_lowercase() };
    let port = if url.port == 0 || (url.secure && url.port == 443) { String::new() } else { format!(":{}", url.port) };
    format!("{}://{host}{port}{}", if url.secure { "https" } else { "http" }, url.path)
}
fn safe_path(path: &str) -> Result<(), String> {
    if !path.starts_with('/') || path.starts_with("//") || path.contains(['?', '#', '\\']) {
        return Err("Unsafe DAV path".into());
    }
    // Decode percent escapes for validation only; '+' in DAV paths is a literal plus.
    let decoded = http::decode(&path.replace('+', "%2B"));
    if decoded.contains(['\\', '\r', '\n', '\0']) || decoded.split('/').any(|s| s == "." || s == "..") || decoded.contains('%') {
        return Err("Unsafe DAV path encoding".into());
    }
    Ok(())
}
fn strong_etag(etag: &str) -> Result<(), String> {
    if etag.len() < 2
        || etag.len() > 1024
        || !etag.starts_with('"')
        || !etag.ends_with('"')
        || etag[1..etag.len() - 1].bytes().any(|b| b < 0x21 || b == b'"' || b == 0x7f)
    {
        return Err("DAV requires a strong ETag for safe synchronization".into());
    }
    Ok(())
}
fn origin(account: u64, href: &str, etag: &str) -> Origin {
    Origin { account, id: href.into(), change_key: etag.into(), ..Default::default() }
}
fn decode(collection: &Collection, href: &str, etag: &str, raw: String, old: Option<&Resource>, zone: Zone) -> Result<Resource, String> {
    let mut result = Resource {
        account: collection.account,
        collection: collection.href.clone(),
        href: href.into(),
        etag: etag.into(),
        raw,
        ..Default::default()
    };
    if collection.contacts {
        result.contacts = mailcraft_vcard::parse(&result.raw);
        if result.contacts.len() != 1 {
            return Err("DAV vCard could not be parsed safely".into());
        }
        for c in &mut result.contacts {
            c.origin = origin(collection.account, href, etag);
            if let Some(before) = old.and_then(|r| r.contacts.first()) {
                c.id = before.id;
                c.favorite = before.favorite;
            }
        }
    } else {
        let parsed = mailcraft_ical::parse(&result.raw, &|t| zone.offset_at(t));
        if parsed.events.is_empty() && !parsed.tasks.is_empty() {
            return Ok(result);
        }
        if parsed.events.is_empty() || !parsed.notes.is_empty() {
            return Err("DAV calendar document has unsupported or invalid event data; retained on server".into());
        }
        result.events = parsed.events;
        for e in &mut result.events {
            e.origin = origin(collection.account, href, etag);
            if let Some(before) = old.and_then(|r| r.events.iter().find(|b| b.uid == e.uid)) {
                e.id = before.id;
                e.calendar = before.calendar;
            }
        }
    }
    Ok(result)
}
fn render_events(events: &[Event], zone: Zone, now: mailcraft_time::Timestamp) -> String {
    mailcraft_ical::write_in(&events.iter().collect::<Vec<_>>(), &[], &mailcraft_ical::WriteOptions { method: String::new(), now }, Some(zone))
}
fn render_contacts(contacts: &[Contact]) -> String {
    mailcraft_vcard::write(&contacts.iter().collect::<Vec<_>>())
}
fn local(old: &Resource, mailbox: &Mailbox) -> Resource {
    let mut resource = old.clone();
    resource.contacts = old.contacts.iter().filter_map(|c| mailbox.contact(c.id).cloned()).collect();
    resource.events = old.events.iter().filter_map(|e| mailbox.event(e.id).cloned()).collect();
    resource
}
fn dirty(old: &Resource, now: &Resource) -> bool {
    old.contacts != now.contacts || old.events != now.events
}
fn equivalent(a: &Resource, b: &Resource) -> bool {
    let clean = |r: &Resource| {
        let mut r = r.clone();
        for c in &mut r.contacts {
            c.origin = Origin::default();
        }
        for e in &mut r.events {
            e.origin = Origin::default();
        }
        (r.contacts, r.events)
    };
    clean(a) == clean(b)
}
fn changed_body(old: &Resource, now: &Resource, zone: Zone) -> Result<String, String> {
    if old.contacts.is_empty() {
        if document::meeting(&old.raw) || now.events.iter().any(|e| e.organizer.is_some() || !e.attendees.is_empty()) {
            return Err("DAV meeting edits are held to prevent automatic invitations; use your provider’s calendar app".into());
        }
        document::patch(&old.raw, &render_events(&old.events, zone, 0), &render_events(&now.events, zone, mailcraft_time::now()), "VEVENT")
    } else {
        document::patch(&old.raw, &render_contacts(&old.contacts), &render_contacts(&now.contacts), "VCARD")
    }
}
fn recover_created(resource: &mut Resource, mailbox: &Mailbox) -> Option<Resource> {
    let mut snapshot =
        Resource { account: resource.account, collection: resource.collection.clone(), href: resource.href.clone(), ..Default::default() };
    if let Some(remote) = resource.contacts.first_mut() {
        let current = mailbox.contacts.values().find(|c| {
            c.origin.account == resource.account
                && c.origin.id == resource.collection
                && !c.uid.is_empty()
                && c.uid == remote.uid
                && resource.href == format!("{}{}.vcf", resource.collection, http::encode(&c.uid))
        })?;
        remote.id = current.id;
        snapshot.contacts.push(remote.clone());
    } else {
        let remote = resource.events.first_mut()?;
        let current = mailbox.events.values().find(|e| {
            e.origin.account == resource.account
                && e.origin.id.is_empty()
                && !e.uid.is_empty()
                && e.uid == remote.uid
                && resource.href == format!("{}{}.ics", resource.collection, http::encode(&e.uid))
        })?;
        remote.id = current.id;
        remote.calendar = current.calendar;
        snapshot.events.push(remote.clone());
    }
    Some(snapshot)
}
/// Authenticate and discover collections without reading items or making any changes. Used
/// by Add Account after mail authentication, with the same in-memory credential.
pub fn check_account(account: &mailcraft_model::Account, password: &str) -> Result<(), String> {
    let clients = clients(account, password)?;
    let collections = collections(&clients, account.id)?;
    if !collections.iter().any(|c| c.contacts) || !collections.iter().any(|c| !c.contacts) {
        return Err("The sign-in did not expose both a calendar and an address book. Check the DAV URLs or continue with mail only.".into());
    }
    Ok(())
}
fn clients(account: &mailcraft_model::Account, password: &str) -> Result<Vec<Client>, String> {
    let separate = !account.carddav_url.is_empty() && account.carddav_url != account.dav_url;
    let mut calendar = Client::new(&account.dav_url, account.dav_login(), password)?;
    calendar.absolute |= separate;
    let mut clients = vec![calendar];
    if separate {
        let mut contacts = Client::new(&account.carddav_url, account.dav_login(), password)?;
        contacts.absolute = true;
        clients.push(contacts);
    }
    Ok(clients)
}
fn collections(clients: &[Client], account: u64) -> Result<Vec<Collection>, String> {
    let mut collections = Vec::new();
    for (i, client) in clients.iter().enumerate() {
        collections.extend(client.discover(account)?.into_iter().filter(|c| clients.len() == 1 || c.contacts == (i == 1)));
        if collections.len() > 256 {
            return Err("Too many DAV collections".into());
        }
    }
    Ok(collections)
}
fn client_for<'a>(collection: &Collection, clients: &'a [Client]) -> Result<&'a Client, String> {
    (if collection.contacts { clients.last() } else { clients.first() }).ok_or_else(|| "DAV connection is unavailable".into())
}
/// Run against the snapshot, returning checkpoints only for verified server representations.
pub fn sync(request: Request) -> Download {
    let mut result = Download {
        account: request.account,
        before: request.mailbox.dav_resources.iter().filter(|r| r.account == request.account).cloned().collect(),
        ..Default::default()
    };
    let Some(account) = request.mailbox.account(request.account) else {
        result.errors.push("DAV account no longer exists".into());
        return result;
    };
    result.url = account.dav_url.clone();
    result.carddav_url = account.carddav_url.clone();
    result.login = Some(account.dav_login().into());
    result.resources = result.before.clone();
    result.snapshots = result.before.iter().map(|r| local(r, &request.mailbox)).collect();
    let run = |result: &mut Download| -> Result<(), String> {
        let clients = clients(account, &request.password)?;
        let zone = Zone::named(&request.mailbox.zone).unwrap_or(Zone::LOCAL);
        result.collections = collections(&clients, account.id)?;
        // Obtain all inventories before any writes. A malformed/partial listing never prunes data.
        let inventories = result.collections.iter().map(|c| client_for(c, &clients)?.inventory(c)).collect::<Result<Vec<_>, _>>()?;
        let mut resources = Vec::new();
        let mut size = 0usize;
        for (collection, inventory) in result.collections.iter().zip(inventories) {
            let client = client_for(collection, &clients)?;
            let old: Vec<_> = result.before.iter().filter(|r| r.collection == collection.href).cloned().collect();
            for (href, etag) in &inventory {
                let before = old.iter().find(|r| r.href == *href);
                let attempt = || -> Result<Option<Resource>, String> {
                    let current = before.map(|r| local(r, &request.mailbox));
                    if let (Some(before), Some(current)) = (before, current.as_ref())
                        && dirty(before, current)
                    {
                        if before.etag != *etag {
                            // The previous PUT may have succeeded before its acknowledgment was
                            // lost. Adopt it only when the server now has precisely our content.
                            let raw = client.fetch(href)?;
                            if client.etag(href)? != *etag {
                                return Err("DAV item changed while checking a conflict".into());
                            }
                            let remote = decode(collection, href, etag, raw, Some(current), zone)?;
                            if equivalent(current, &remote) {
                                return Ok(Some(remote));
                            }
                            return Err("DAV conflict: both local and server copies changed; local edits retained".into());
                        }
                        if request.read_only {
                            return Ok(Some(before.clone()));
                        }
                        if collection.read_only {
                            return Err("DAV collection is read-only; local edits retained".into());
                        }
                        let deleting = current.contacts.is_empty() && current.events.is_empty();
                        if deleting && document::meeting(&before.raw) {
                            return Err(
                                "DAV meeting deletion is held to prevent automatic cancellation mail; use your provider’s calendar app".into()
                            );
                        }
                        let url = client.url(href, &collection.href)?;
                        if deleting {
                            client.call(&url, "DELETE", None, &[("If-Match", etag)])?;
                            return Ok(None);
                        }
                        let body = changed_body(before, current, zone)?;
                        client.call(
                            &url,
                            "PUT",
                            Some(&body),
                            &[
                                ("If-Match", etag),
                                ("Content-Type", if collection.contacts { "text/vcard; charset=utf-8" } else { "text/calendar; charset=utf-8" }),
                            ],
                        )?;
                        let tag = client.etag(href)?;
                        let raw = client.fetch(href)?;
                        if client.etag(href)? != tag {
                            return Err("DAV item changed during write verification; local edits retained".into());
                        }
                        let verified = decode(collection, href, &tag, raw, Some(current), zone)?;
                        return Ok(Some(verified));
                    }
                    if let Some(before) = before
                        && before.etag == *etag
                    {
                        return Ok(Some(before.clone()));
                    }
                    let raw = client.fetch(href)?;
                    // PROPFIND/GET races must not pair an old ETag with newer bytes.
                    if client.etag(href)? != *etag {
                        return Err("DAV item changed during download; retry required".into());
                    }
                    decode(collection, href, etag, raw, before, zone).map(Some)
                };
                match attempt() {
                    Ok(mut value) => {
                        if before.is_none()
                            && let Some(resource) = &mut value
                            && let Some(snapshot) = recover_created(resource, &request.mailbox)
                        {
                            result.written.push(resource.href.clone());
                            result.before.push(snapshot.clone());
                            result.snapshots.push(snapshot);
                        }
                        if value.is_none() {
                            result.removed.push(href.clone());
                            result.resources.retain(|r| r.href != *href);
                        } else if let Some(verified) = &value {
                            result.resources.retain(|r| r.href != *href);
                            result.resources.push(verified.clone());
                        }
                        if before.is_some_and(|b| dirty(b, &local(b, &request.mailbox))) && !request.read_only {
                            result.uploaded += 1;
                            result.written.push(href.clone());
                        }
                        if let Some(value) = value {
                            size += value.raw.len();
                            resources.push(value);
                        }
                    }
                    Err(error) => {
                        result.errors.push(error);
                        if let Some(before) = before {
                            resources.push(before.clone());
                        } else {
                            return Err("DAV download incomplete; existing items retained".into());
                        }
                    }
                }
                if size > 64 * 1024 * 1024 || resources.len() > MAX_ITEMS {
                    return Err("DAV account exceeds sync limits".into());
                }
            }
            // A server deletion conflicts with a local edit; preserve the local copy and checkpoint.
            for before in old.iter().filter(|r| !inventory.contains_key(&r.href)) {
                let current = local(before, &request.mailbox);
                if dirty(before, &current) && (!current.contacts.is_empty() || !current.events.is_empty()) {
                    resources.push(before.clone());
                    result.errors.push("DAV conflict: server deleted an item edited locally; local copy retained".into());
                }
            }
            if !request.read_only && !collection.read_only {
                let existing_ids: HashSet<u64> =
                    result.before.iter().flat_map(|r| r.contacts.iter().map(|c| c.id).chain(r.events.iter().map(|e| e.id))).collect();
                let mut made = Vec::new();
                if collection.contacts {
                    for c in request
                        .mailbox
                        .contacts
                        .values()
                        .filter(|c| c.origin.account == account.id && c.origin.id == collection.href && !existing_ids.contains(&c.id))
                    {
                        made.push(Resource {
                            account: account.id,
                            collection: collection.href.clone(),
                            contacts: vec![(**c).clone()],
                            ..Default::default()
                        });
                    }
                } else {
                    let calendar = request.mailbox.calendars.iter().find(|c| c.origin.account == account.id && c.origin.id == collection.href);
                    if let Some(calendar) = calendar {
                        for e in request.mailbox.events.values().filter(|e| {
                            e.calendar == calendar.id && e.origin.account == account.id && e.origin.id.is_empty() && !existing_ids.contains(&e.id)
                        }) {
                            made.push(Resource {
                                account: account.id,
                                collection: collection.href.clone(),
                                events: vec![(**e).clone()],
                                ..Default::default()
                            });
                        }
                    }
                }
                for mut new in made {
                    let mut attempt = || -> Result<Resource, String> {
                        let (uid, body, extension) = if let Some(c) = new.contacts.first() {
                            (&c.uid, render_contacts(&new.contacts), "vcf")
                        } else if let Some(e) = new.events.first() {
                            (&e.uid, render_events(&new.events, zone, mailcraft_time::now()), "ics")
                        } else {
                            return Err("Empty DAV item".into());
                        };
                        if uid.is_empty() {
                            return Err("New DAV item needs a stable UID".into());
                        }
                        if extension == "ics" && document::meeting(&body) {
                            return Err("DAV meeting creation is held to prevent automatic invitations; use your provider’s calendar app".into());
                        }
                        let href = format!("{}{}.{}", collection.href, http::encode(uid), extension);
                        let url = client.url(&href, &collection.href)?;
                        client.call(
                            &url,
                            "PUT",
                            Some(&body),
                            &[
                                ("If-None-Match", "*"),
                                ("Content-Type", if collection.contacts { "text/vcard; charset=utf-8" } else { "text/calendar; charset=utf-8" }),
                            ],
                        )?;
                        let etag = client.etag(&href)?;
                        let raw = client.fetch(&href)?;
                        if client.etag(&href)? != etag {
                            return Err("DAV item changed during creation verification; local item retained".into());
                        }
                        new.href = href.clone();
                        decode(collection, &href, &etag, raw, Some(&new), zone)
                    };
                    match attempt() {
                        Ok(value) => {
                            result.before.push(new.clone());
                            result.snapshots.push(new.clone());
                            result.written.push(value.href.clone());
                            result.resources.push(value.clone());
                            resources.push(value);
                            result.uploaded += 1;
                        }
                        Err(error) => result.errors.push(error),
                    }
                }
            }
        }
        for before in &result.before {
            if !result.collections.iter().any(|c| c.href == before.collection) {
                resources.push(before.clone());
                result.errors.push("A DAV collection is unavailable; its local items were retained".into());
            }
        }
        result.resources = resources;
        result.complete = true;
        Ok(())
    };
    if let Err(error) = run(&mut result) {
        result.errors.push(error);
        result.complete = false;
    }
    result.errors.sort();
    result.errors.dedup();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeHttp;
    #[test]
    fn redirect_targets_are_checked_before_credentials_and_writes_are_never_redirected() {
        let target = FakeHttp::answering(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nsafe".to_vec());
        let redirect = format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: {}\r\nContent-Length: 0\r\n\r\n", target.url("/home/"));
        let source = FakeHttp::answering(redirect.as_bytes().to_vec());
        let mut client = Client::new(&source.url("/"), "u", "fake-secret").unwrap();
        assert!(client.call(&client.base, "GET", None, &[]).is_err());
        assert_eq!(target.asked(), 0);
        client.trusted_test_origins.push(Url::parse(&target.url("/")).unwrap());
        let (reply, at) = client.call_at(&client.base, "GET", None, &[]).unwrap();
        assert_eq!(reply.body, b"safe");
        assert_eq!(at.path, "/home/");
        assert!(target.request(0).contains("Authorization: Basic "));
        assert!(client.call(&client.base, "PUT", Some("data"), &[("If-Match", "\"1\"")]).is_err());
        assert_eq!(target.asked(), 1);
        let foreign = FakeHttp::answering(redirect.as_bytes().to_vec());
        let mut blocked = Client::new(&foreign.url("/"), "u", "fake-secret").unwrap();
        blocked.trusted_test_origins.push(Url::parse(&source.url("/")).unwrap());
        assert!(blocked.call(&blocked.base, "GET", None, &[]).is_err());
        assert_eq!(target.asked(), 1);
    }
    #[test]
    fn redirect_loops_stop_and_relative_hrefs_use_the_effective_partition() {
        let source = FakeHttp::answering(b"HTTP/1.1 307 Temporary Redirect\r\nLocation: /loop\r\nContent-Length: 0\r\n\r\n".to_vec());
        let mut c = Client::new(&source.url("/"), "u", "fake-secret").unwrap();
        c.trusted_test_origins.push(c.base.clone());
        assert!(c.call(&c.base, "PROPFIND", Some("xml"), &[]).unwrap_err().contains("loop"));
        assert_eq!(source.asked(), 2);
        let c = Client::new("https://caldav.icloud.com/", "u", "fake-secret").unwrap();
        let shard = Url::parse("https://p43-caldav.icloud.com/123/home/").unwrap();
        let at = c.url_at("calendar/", &shard).unwrap();
        assert_eq!(c.href(&at), "https://p43-caldav.icloud.com/123/home/calendar/");
        assert!(c.url_at("https://p43-contacts.icloud.com/123/home/", &shard).is_err());
    }
    #[test]
    fn never_forwards_credentials_to_foreign_or_unsafe_hrefs() {
        let c = Client::new("https://example.test/dav/", "u", "secret").unwrap();
        for href in ["https://evil.test/x", "http://example.test/x", "//evil.test/x", "/dav/%2e%2e/secret", "/dav/a?x", "/dav/%250a"] {
            assert!(c.url(href, "/dav/").is_err(), "{href}");
        }
        assert_eq!(c.url("card.vcf", "/dav/book/").unwrap().path, "/dav/book/card.vcf");
        assert!(strong_etag("W/\"a\"").is_err());
        assert!(strong_etag("\"a\"\r\nInjected:x").is_err());
    }
}
#[cfg(test)]
mod standin_tests;
