//! Where well-known mail providers keep their servers and how their owners sign in, worked out
//! from an address's domain or from the domain's mail exchangers.
//!
//! Plain data: nothing is looked up here. A host with sockets asks the DNS for the exchangers and
//! passes the answer in, which is how an address at a company's own domain is recognised as
//! (say) Microsoft 365. The server names are the ones each provider publishes.

use crate::{OAuthProvider, Security, Server};

/// What an account at a provider usually takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    /// The provider's name (`Microsoft 365`); empty for a guess.
    pub service: &'static str,
    /// Set when the owner signs in on the provider's own page instead of with a password.
    pub oauth: Option<OAuthProvider>,
    pub incoming: Server,
    pub outgoing: Server,
    /// Something the owner should know before connecting; may be empty.
    pub note: &'static str,
    /// A combined CalDAV/CardDAV context, when the provider publishes one.
    pub dav_url: &'static str,
    pub carddav_url: &'static str,
}

/// Domain-only discovery results. No identity or credential belongs in this data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Discovery {
    /// Confirmed by the fixed Despres Connect HTTPS discovery service, never by DNS alone.
    pub despres_connect: bool,
    pub mx: Vec<String>,
    pub incoming: Option<Server>,
    pub outgoing: Option<Server>,
    pub dav_url: String,
    pub carddav_url: String,
    pub note: String,
}

const APP_PASSWORD: &str = "This provider wants an app password (made in the account's security settings), not the everyday one.";
const MICROSOFT: &str = "Microsoft accounts sign in on Microsoft's own page; no password is typed here.";

fn server(host: &str, port: u16, security: Security) -> Server {
    Server { host: host.to_string(), port, security }
}

fn with_password(service: &'static str, imap: &str, smtp: (&str, u16, Security), note: &'static str) -> Preset {
    Preset {
        service,
        oauth: None,
        incoming: server(imap, 993, Security::Tls),
        outgoing: server(smtp.0, smtp.1, smtp.2),
        note,
        dav_url: "",
        carddav_url: "",
    }
}

/// Apple publishes separate calendar and contacts services. The same app-specific password
/// is used, with TLS and a service-specific allowlist for authenticated partition redirects.
fn icloud() -> Preset {
    let mut p = with_password(
        "iCloud",
        "imap.mail.me.com",
        ("smtp.mail.me.com", 587, Security::StartTls),
        "Use an Apple app-specific password, not your Apple Account password. If your Apple Account sign-in differs, enter it in Change account settings.",
    );
    p.dav_url = "https://caldav.icloud.com/";
    p.carddav_url = "https://contacts.icloud.com/";
    p
}

/// Outlook.com (`consumer`) or Microsoft 365 work and school mail.
pub fn microsoft(consumer: bool) -> Preset {
    // Outlook.com sends through its own host; work and school mail through Office 365's.
    let (service, smtp) = if consumer { ("Outlook.com", "smtp-mail.outlook.com") } else { ("Microsoft 365", "smtp.office365.com") };
    Preset {
        service,
        oauth: Some(OAuthProvider::Microsoft),
        incoming: server("outlook.office365.com", 993, Security::Tls),
        outgoing: server(smtp, 587, Security::StartTls),
        note: MICROSOFT,
        dav_url: "",
        carddav_url: "",
    }
}

/// A host name fit to look up or connect to: letters, digits, dots and dashes, with a dot in it.
pub fn is_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.contains('.')
        && domain.split('.').all(|label| !label.is_empty() && label.len() <= 63 && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// Mail Microsoft hosts for anyone: Outlook.com and its older names, in every country
/// (`outlook.com`, `hotmail.co.uk`, `live.com.au`; not `live.somecompany.example`).
fn is_microsoft_consumer(domain: &str) -> bool {
    let mut labels = domain.split('.');
    let first = labels.next().unwrap_or_default();
    let rest: Vec<&str> = labels.collect();
    matches!(first, "outlook" | "hotmail" | "live" | "msn") && (1..=2).contains(&rest.len()) && rest.iter().all(|l| (2..=3).contains(&l.len()))
}

/// From the address's domain alone (`gmail.com`). `None` for a domain that isn't a provider's own.
pub fn by_domain(domain: &str) -> Option<Preset> {
    let domain = domain.trim().to_ascii_lowercase();
    if !is_domain(&domain) {
        return None;
    }
    if is_microsoft_consumer(&domain) {
        return Some(microsoft(true));
    }
    let (tls, starttls) = (Security::Tls, Security::StartTls);
    Some(match domain.as_str() {
        "despresai.com" | "badmintonsoldiers.club" => Preset {
            service: "Despres Connect",
            oauth: None,
            incoming: server("outlook.despresai.com", 9933, tls),
            outgoing: server(&format!("mail.{domain}"), 587, starttls),
            dav_url: "https://outlook.despresai.com/dav/",
            carddav_url: "",
            note: "Mail, calendar and contacts use your Despres Connect sign-in.",
        },
        "gmail.com" | "googlemail.com" => with_password("Gmail", "imap.gmail.com", ("smtp.gmail.com", 465, tls), APP_PASSWORD),
        "icloud.com" | "me.com" | "mac.com" => icloud(),
        "fastmail.com" | "fastmail.fm" => with_password("Fastmail", "imap.fastmail.com", ("smtp.fastmail.com", 465, tls), APP_PASSWORD),
        "aol.com" => with_password("AOL", "imap.aol.com", ("smtp.aol.com", 465, tls), APP_PASSWORD),
        "zoho.com" | "zohomail.com" => with_password("Zoho", "imap.zoho.com", ("smtp.zoho.com", 465, tls), ""),
        "gmx.com" | "gmx.us" => with_password("GMX", "imap.gmx.com", ("mail.gmx.com", 587, starttls), ""),
        "gmx.net" | "gmx.de" | "gmx.at" | "gmx.ch" => with_password("GMX", "imap.gmx.net", ("mail.gmx.net", 587, starttls), ""),
        "mail.com" => with_password("mail.com", "imap.mail.com", ("smtp.mail.com", 587, starttls), ""),
        d if d == "ymail.com" || d == "rocketmail.com" || d.starts_with("yahoo.") => {
            with_password("Yahoo", "imap.mail.yahoo.com", ("smtp.mail.yahoo.com", 465, tls), APP_PASSWORD)
        }
        _ => return None,
    })
}

/// From one of the domain's mail exchangers (`example-org.mail.protection.outlook.com`): who
/// hosts the mail of a domain that isn't the provider's own.
pub fn by_mx(host: &str) -> Option<Preset> {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if !is_domain(&host) {
        return None;
    }
    // The host itself or anything under it, never a look-alike (`evilgoogle.com`).
    let under = |parent: &str| host == parent || host.ends_with(&format!(".{parent}"));
    let tls = Security::Tls;
    if under("olc.protection.outlook.com") {
        return Some(microsoft(true));
    }
    if under("mail.protection.outlook.com") || under("mx.microsoft") {
        return Some(microsoft(false));
    }
    if under("google.com") || under("googlemail.com") {
        return Some(with_password("Google Workspace", "imap.gmail.com", ("smtp.gmail.com", 465, tls), APP_PASSWORD));
    }
    if under("yahoodns.net") {
        return Some(with_password("Yahoo", "imap.mail.yahoo.com", ("smtp.mail.yahoo.com", 465, tls), APP_PASSWORD));
    }
    if under("mail.icloud.com") {
        return Some(icloud());
    }
    if under("messagingengine.com") {
        return Some(with_password("Fastmail", "imap.fastmail.com", ("smtp.fastmail.com", 465, tls), APP_PASSWORD));
    }
    if under("zoho.com") {
        return Some(with_password("Zoho", "imap.zoho.com", ("smtp.zoho.com", 465, tls), ""));
    }
    None
}

/// From the provider's name, for an owner who says whose the account is (`microsoft365`,
/// `outlook.com`, `google`, `yahoo`, `icloud`, `fastmail`).
pub fn by_name(name: &str) -> Option<Preset> {
    let name: String = name.trim().to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    match name.as_str() {
        "microsoft" | "microsoft365" | "office365" | "m365" | "exchange" => Some(microsoft(false)),
        "outlook" | "outlookcom" | "hotmail" | "live" => Some(microsoft(true)),
        "google" | "gmail" | "googleworkspace" => by_domain("gmail.com"),
        "yahoo" => by_domain("yahoo.com"),
        "icloud" | "apple" => by_domain("icloud.com"),
        "fastmail" => by_domain("fastmail.com"),
        "aol" => by_domain("aol.com"),
        "zoho" => by_domain("zoho.com"),
        "despres" | "despresconnect" => by_domain("despresai.com"),
        _ => None,
    }
}

/// The usual names at a domain nobody here has heard of, for the owner to correct.
pub fn guess(domain: &str) -> Preset {
    let domain = domain.trim().to_ascii_lowercase();
    Preset {
        service: "",
        oauth: None,
        incoming: server(&format!("imap.{domain}"), 993, Security::Tls),
        outgoing: server(&format!("smtp.{domain}"), 587, Security::StartTls),
        note: "",
        dav_url: "",
        carddav_url: "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_providers_own_domains() {
        for domain in ["outlook.com", "Hotmail.co.uk", "live.com.au", "msn.com", " outlook.de "] {
            let p = by_domain(domain).unwrap();
            assert_eq!(
                (p.service, p.oauth, p.outgoing.host.as_str()),
                ("Outlook.com", Some(OAuthProvider::Microsoft), "smtp-mail.outlook.com"),
                "{domain}"
            );
        }
        assert_eq!(by_domain("gmail.com").unwrap().incoming, server("imap.gmail.com", 993, Security::Tls));
        assert_eq!(by_domain("yahoo.co.uk").unwrap().service, "Yahoo");
        assert_eq!(by_domain("gmx.net").unwrap().outgoing, server("mail.gmx.net", 587, Security::StartTls));
        // Someone else's domain that merely starts like one of them, and things that aren't domains.
        for domain in ["live.somecompany.example", "outlook.example.org", "hotmail", "lumen.example", "", "a b.com", "evil/../x.com"] {
            assert_eq!(by_domain(domain), None, "{domain}");
        }
    }

    #[test]
    fn a_companys_domain_is_known_by_who_takes_its_mail() {
        let work = by_mx("example-org.mail.protection.outlook.com.").unwrap();
        assert_eq!((work.service, work.oauth), ("Microsoft 365", Some(OAuthProvider::Microsoft)));
        assert_eq!(
            (work.incoming.host.as_str(), work.outgoing.host.as_str(), work.outgoing.port),
            ("outlook.office365.com", "smtp.office365.com", 587)
        );
        assert_eq!(by_mx("example-org.h-v1.mx.microsoft").unwrap().service, "Microsoft 365");
        assert_eq!(by_mx("OUTLOOK-COM.olc.protection.outlook.com").unwrap().service, "Outlook.com");
        assert_eq!(by_mx("aspmx.l.google.com").unwrap().service, "Google Workspace");
        assert_eq!(by_mx("in1-smtp.messagingengine.com").unwrap().service, "Fastmail");
        assert_eq!(by_mx("mx01.mail.icloud.com").unwrap().service, "iCloud");
        // A look-alike is not the provider, nor is a host that isn't one.
        for host in ["evilgoogle.com", "mail.protection.outlook.com.evil.example", "mx.lumen.example", "", "a b", "x/../mx.microsoft"] {
            assert_eq!(by_mx(host), None, "{host}");
        }
    }

    #[test]
    fn a_provider_by_name() {
        assert_eq!(by_name(" Microsoft 365 ").unwrap().service, "Microsoft 365");
        assert_eq!(by_name("Outlook.com").unwrap().service, "Outlook.com");
        assert_eq!(by_name("google").unwrap().incoming.host, "imap.gmail.com");
        assert_eq!(by_name("iCloud").unwrap().service, "iCloud");
        assert_eq!(by_name("nobody"), None);
        assert_eq!(by_name(""), None);
    }

    #[test]
    fn a_guess_uses_the_usual_names() {
        let p = guess("Lumen.Example");
        assert_eq!((p.service, p.incoming.host.as_str(), p.outgoing.host.as_str()), ("", "imap.lumen.example", "smtp.lumen.example"));
        assert!(is_domain("mail.lumen.example") && !is_domain("localhost") && !is_domain(&format!("{}.example", "a".repeat(64))));
    }
}
