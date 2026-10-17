//! Email-first setup using domain-only DNS service discovery (RFC 6186/8314/6764).
//! No password, mailbox name, authentication probe, or mail is sent by discovery.
use crate::{
    NetError,
    dns_service::{self, Service},
};
use mailcraft_model::{
    Security, Server,
    provider::{self, Discovery},
};

mod despres_connect;

const LABELS: [&str; 5] = ["_imaps._tcp", "_submissions._tcp", "_submission._tcp", "_caldavs._tcp", "_carddavs._tcp"];

/// Discover encrypted mail services and a shared DAV context. Provider presets take priority
/// in the UI. Unknown domains can publish SRV and DAV TXT `path` records. Independent DNS
/// questions run together; a dropped wizard never starts a sign-in or creates an account.
pub fn discover(domain: &str) -> Result<Discovery, NetError> {
    if !provider::is_domain(domain) {
        return Err(NetError::Config("Account discovery needs a domain name".into()));
    }
    let domain = domain.to_ascii_lowercase();
    if despres_connect::lookup(&domain).unwrap_or(false) {
        // Membership comes from our fixed HTTPS service. Its reply supplies no endpoints.
        // Keep credentials and the full address out of this request.
        return Ok(Discovery { despres_connect: true, ..Default::default() });
    }
    std::thread::scope(|scope| {
        let mx = std::thread::Builder::new().name("setup MX".into()).spawn_scoped(scope, || crate::lookup_mx(&domain));
        let jobs: Vec<_> = LABELS
            .iter()
            .map(|label| std::thread::Builder::new().name("setup service".into()).spawn_scoped(scope, || dns_service::lookup(&domain, label)))
            .collect();
        let services: Vec<_> = jobs.into_iter().map(|job| job.ok().and_then(|job| job.join().ok()).and_then(Result::ok).flatten()).collect();
        let mx = mx.ok().and_then(|job| job.join().ok()).and_then(Result::ok).unwrap_or_default();
        Ok(settings(mx, &services))
    })
}

fn settings(mx: Vec<String>, services: &[Option<Service>]) -> Discovery {
    let at = |i| services.get(i).and_then(Option::as_ref);
    let server = |s: &Service, security| Server { host: s.host.clone(), port: s.port, security };
    let mut found = Discovery {
        mx,
        incoming: at(0).map(|s| server(s, Security::Tls)),
        outgoing: at(1).map(|s| server(s, Security::Tls)).or_else(|| at(2).map(|s| server(s, Security::StartTls))),
        ..Default::default()
    };
    let dav = |s: &Service| s.path.as_ref().map(|path| format!("https://{}:{}{path}", s.host, s.port));
    match (at(3).and_then(dav), at(4).and_then(dav)) {
        (Some(cal), Some(book)) => {
            if cal != book {
                found.carddav_url = book;
            }
            found.dav_url = cal;
        }
        (None, None) => {}
        _ => found.note = "Mail settings were checked. Calendar and contacts need their DAV URLs in Change account settings.".into(),
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    fn service(host: &str, port: u16, path: Option<&str>) -> Option<Service> {
        Some(Service { host: host.into(), port, path: path.map(str::to_string) })
    }
    #[test]
    fn encrypted_services_connect_all_three_without_guessing_a_mailbox_path() {
        let found = settings(
            vec![],
            &[
                service("mail.example.org", 9993, None),
                service("mail.example.org", 465, None),
                service("mail.example.org", 587, None),
                service("group.example.org", 443, Some("/groupware/")),
                service("group.example.org", 443, Some("/groupware/")),
            ],
        );
        assert_eq!(found.incoming.unwrap().port, 9993);
        assert_eq!(found.outgoing.unwrap().security, Security::Tls);
        assert_eq!(found.dav_url, "https://group.example.org:443/groupware/");
        assert!(found.note.is_empty());
    }
    #[test]
    fn separate_dav_services_keep_both_origins_and_missing_services_remain_manual() {
        let found = settings(
            vec![],
            &[
                None,
                None,
                service("mail.example.org", 587, None),
                service("cal.example.org", 443, Some("/dav/")),
                service("people.example.org", 443, Some("/dav/")),
            ],
        );
        assert_eq!(found.dav_url, "https://cal.example.org:443/dav/");
        assert_eq!(found.carddav_url, "https://people.example.org:443/dav/");
        assert!(found.note.is_empty());
        assert_eq!(found.outgoing.unwrap().security, Security::StartTls);
        assert!(settings(vec![], &[]).dav_url.is_empty());
        assert!(discover("person@example.org").is_err());
    }
}
