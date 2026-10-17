//! Deliberately narrow Apple service boundaries, independent of an account's email domain.
//! An arbitrary domain, generic *.icloud.com host, URL query, port or TLS downgrade is not
//! authority to receive an iCloud app-specific password.
use crate::http::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Service {
    Calendar,
    Contacts,
}
impl Service {
    pub(super) fn of(url: &Url) -> Option<Self> {
        [Self::Calendar, Self::Contacts].into_iter().find(|s| s.allows(url))
    }
    pub(super) fn allows(self, url: &Url) -> bool {
        if !url.secure || !matches!(url.port, 0 | 443) {
            return false;
        }
        let host = url.host.to_ascii_lowercase();
        let names: &[&str] = match self {
            Self::Calendar => &["caldav", "calendars"],
            Self::Contacts => &["contacts", "carddav"],
        };
        names.iter().any(|name| {
            if host == format!("{name}.icloud.com") {
                return true;
            }
            host.strip_prefix('p')
                .and_then(|h| h.strip_suffix(&format!("-{name}.icloud.com")))
                .is_some_and(|partition| !partition.is_empty() && partition.len() <= 3 && partition.bytes().all(|b| b.is_ascii_digit()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn apple_partition_trust_is_service_specific_https_and_exactly_bounded() {
        for good in ["https://caldav.icloud.com/", "https://p43-caldav.icloud.com:443/123/principal/", "https://p01-calendars.icloud.com/"] {
            assert!(Service::Calendar.allows(&Url::parse(good).unwrap()));
        }
        assert!(Service::Contacts.allows(&Url::parse("https://p69-contacts.icloud.com/").unwrap()));
        for bad in [
            "https://p43-caldav.icloud.com.evil.example/",
            "https://evilicloud.com/",
            "https://apple.com/",
            "https://setup.icloud.com/",
            "https://p43-contacts.icloud.com/",
            "https://px-caldav.icloud.com/",
            "https://caldav.icloud.com:444/",
            "http://127.0.0.1/",
        ] {
            assert!(!Service::Calendar.allows(&Url::parse(bad).unwrap()), "{bad}");
        }
    }
}
