//! Bounded domain-only SRV/TXT lookups. Only the setup service labels below are accepted.
use crate::{
    NetError,
    http::{self, Url},
};
use mailcraft_model::provider;
use ring::rand::SecureRandom;
use serde_json::Value;
use std::time::Duration;

const RESOLVERS: [&str; 2] = ["https://cloudflare-dns.com/dns-query", "https://dns.google/resolve"];
const LABELS: [&str; 5] = ["_imaps._tcp", "_submissions._tcp", "_submission._tcp", "_caldavs._tcp", "_carddavs._tcp"];
#[derive(Clone, Debug)]
pub(crate) struct Service {
    pub host: String,
    pub port: u16,
    pub path: Option<String>,
}

pub(crate) fn lookup(domain: &str, label: &str) -> Result<Option<Service>, NetError> {
    lookup_at(&RESOLVERS, domain, label)
}
fn lookup_at(resolvers: &[&str], domain: &str, label: &str) -> Result<Option<Service>, NetError> {
    if !provider::is_domain(domain) || !LABELS.contains(&label) {
        return Err(NetError::Config("Invalid discovery domain or service".into()));
    }
    let name = format!("{label}.{}", domain.to_ascii_lowercase());
    let mut bytes = [0; 8];
    let _ = ring::rand::SystemRandom::new().fill(&mut bytes);
    let Some(mut service) = srv(&records(resolvers, &name, "SRV", 33)?, u64::from_le_bytes(bytes)) else { return Ok(None) };
    if label == "_caldavs._tcp" || label == "_carddavs._tcp" {
        service.path = records(resolvers, &name, "TXT", 16).ok().and_then(|rows| path(&rows));
    }
    Ok(Some(service))
}
fn records(resolvers: &[&str], name: &str, kind: &str, number: u64) -> Result<Vec<String>, NetError> {
    for endpoint in resolvers.iter().take(2) {
        let query = http::form(&[("name", name), ("type", kind)]);
        let url = Url::parse(&format!("{endpoint}{}{query}", if endpoint.contains('?') { '&' } else { '?' }))?;
        let Ok(answer) = http::get(&url, &[("Accept", "application/dns-json")], Duration::from_secs(8)) else { continue };
        if answer.status != 200 {
            continue;
        }
        let Ok(json) = serde_json::from_slice::<Value>(&answer.body) else { continue };
        match json.get("Status").and_then(Value::as_u64) {
            Some(3) => return Ok(Vec::new()),
            Some(0) => {}
            _ => continue,
        }
        return Ok(json
            .get("Answer")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(5000)
            .filter(|row| {
                row.get("type").and_then(Value::as_u64) == Some(number)
                    && row.get("name").and_then(Value::as_str).is_some_and(|n| n.trim_end_matches('.').eq_ignore_ascii_case(name))
            })
            .filter_map(|row| row.get("data").and_then(Value::as_str))
            .filter(|text| text.len() <= 1024)
            .take(64)
            .map(str::to_string)
            .collect());
    }
    Err(NetError::Config("Service discovery could not reach a DNS resolver".into()))
}
fn srv(rows: &[String], roll: u64) -> Option<Service> {
    let mut choices = Vec::new();
    for row in rows.iter().take(64) {
        let mut words = row.split_whitespace();
        let (Some(priority), Some(weight), Some(port), Some(host), None) = (
            words.next().and_then(|n| n.parse::<u16>().ok()),
            words.next().and_then(|n| n.parse::<u16>().ok()),
            words.next().and_then(|n| n.parse::<u16>().ok()),
            words.next(),
            words.next(),
        ) else {
            continue;
        };
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        if port == 0
            || host.parse::<std::net::IpAddr>().is_ok()
            || !provider::is_domain(&host)
            || host.split('.').any(|label| label.starts_with('-') || label.ends_with('-'))
        {
            continue;
        }
        choices.push((priority, weight, Service { host, port, path: None }));
    }
    let priority = choices.iter().map(|c| c.0).min()?;
    choices.retain(|c| c.0 == priority);
    let total: u64 = choices.iter().map(|c| u64::from(c.1)).sum();
    if total == 0 {
        let index = (roll % choices.len() as u64) as usize;
        return choices.get(index).map(|c| c.2.clone());
    }
    let mut selected = roll % total;
    for (_, weight, service) in choices {
        if selected < u64::from(weight) {
            return Some(service);
        }
        selected -= u64::from(weight);
    }
    None
}
/// DNS presentation-format character strings. Only a safe absolute context path is useful.
fn path(rows: &[String]) -> Option<String> {
    for row in rows.iter().take(64) {
        let mut fields = Vec::new();
        let mut chars = row.chars().peekable();
        let mut valid = true;
        while let Some(c) = chars.next() {
            if c.is_ascii_whitespace() {
                continue;
            }
            if c != '"' {
                valid = false;
                break;
            }
            let mut closed = false;
            let mut text = String::new();
            for c in chars.by_ref() {
                if c == '"' {
                    closed = true;
                    break;
                }
                if c == '\\' {
                    valid = false;
                    break;
                }
                text.push(c);
            }
            if !closed {
                valid = false;
                break;
            }
            fields.push(text);
        }
        if !valid {
            continue;
        }
        let paths: Vec<_> =
            fields.iter().filter_map(|text| text.split_once('=')).filter(|(key, _)| key.eq_ignore_ascii_case("path")).map(|(_, path)| path).collect();
        let [path] = paths.as_slice() else { continue };
        if path.is_ascii()
            && path.len() <= 1000
            && path.starts_with('/')
            && !path.starts_with("//")
            && !path.contains(['?', '#', '\\', '%'])
            && !path.chars().any(|c| c.is_control() || c.is_whitespace())
            && !path.split('/').any(|p| p == "." || p == "..")
        {
            return Some((*path).into());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeHttp, json_answer};
    #[test]
    fn service_dns_sends_only_domain_and_checks_record_owner_and_priority() {
        let server = FakeHttp::answering(json_answer(
            "200 OK",
            r#"{"Status":0,"Answer":[
            {"name":"_imaps._tcp.example.org.","type":33,"data":"20 0 993 backup.example.org."},
            {"name":"_imaps._tcp.example.org.","type":33,"data":"10 0 9993 imap.example.org."},
            {"name":"unrelated.example.org.","type":33,"data":"0 0 993 evil.example.org."}] }"#,
        ));
        let service = lookup_at(&[&server.url("/dns")], "example.org", "_imaps._tcp").unwrap().unwrap();
        assert_eq!((service.host.as_str(), service.port), ("imap.example.org", 9993));
        let request = server.request(0);
        assert!(request.starts_with("GET /dns?name=_imaps._tcp.example.org&type=SRV "));
        assert!(!request.contains("Authorization") && !request.contains('@'));
        assert!(lookup_at(&[&server.url("/dns")], "a@example.org", "_imaps._tcp").is_err());
        assert_eq!(server.asked(), 1);
    }
    #[test]
    fn weighted_choices_and_hostile_service_paths_are_bounded() {
        let rows = vec!["10 1 993 a.example.org.".into(), "10 3 993 b.example.org.".into(), "20 9 993 c.example.org.".into()];
        assert_eq!(srv(&rows, 0).unwrap().host, "a.example.org");
        assert_eq!(srv(&rows, 1).unwrap().host, "b.example.org");
        assert!(srv(&["0 0 0 .".into(), "0 0 993 bad/host".into()], 0).is_none());
        assert_eq!(path(&["\"txtvers=1\" \"Path=/group/dav/\"".into()]), Some("/group/dav/".into()));
        for bad in [
            "\"path=//evil.example/\"",
            "\"path=/../dav\"",
            "\"path=/%2e%2e/dav\"",
            "\"path=/dav?password=x\"",
            "\"path=/dav\\032/\"",
            "path=/dav/",
            "\"path=/dav/\"broken",
        ] {
            assert!(path(&[bad.into()]).is_none(), "{bad}");
        }
    }
}
