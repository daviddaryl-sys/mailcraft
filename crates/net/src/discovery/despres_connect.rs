//! Confirm a tenant domain at our fixed HTTPS service; never accept endpoints from its reply.
use std::time::Duration;

use crate::{NetError, http};
use mailcraft_model::provider;
use mailcraft_proto::ProtoError;

const ENDPOINT: &str = "https://outlook.despresai.com/.well-known/mailcraft";

pub(super) fn lookup(domain: &str) -> Result<bool, NetError> {
    lookup_at(ENDPOINT, domain)
}

fn lookup_at(endpoint: &str, domain: &str) -> Result<bool, NetError> {
    if !provider::is_domain(domain) {
        return Err(NetError::Config("Despres Connect discovery needs a domain name".into()));
    }
    let domain = domain.to_ascii_lowercase();
    let url = http::Url::parse(&format!("{endpoint}?{}", http::form(&[("domain", &domain)])))?;
    let reply = http::request(
        &url,
        &http::Request { method: "GET", headers: &[("Accept", "application/json")], body: None, patience: Duration::from_secs(4), max_body: 1024 },
    )?;
    if reply.status == 404 {
        return Ok(false);
    }
    if reply.status != 200 {
        return Err(NetError::Proto(ProtoError::Rejected(format!("Despres Connect discovery returned HTTP {}", reply.status))));
    }
    let value: serde_json::Value = serde_json::from_slice(&reply.body).map_err(|_| invalid())?;
    if value.get("version").and_then(serde_json::Value::as_u64) != Some(1)
        || value.get("provider").and_then(serde_json::Value::as_str) != Some("despres-connect")
        || value.get("domain").and_then(serde_json::Value::as_str) != Some(domain.as_str())
    {
        return Err(invalid());
    }
    value.get("hosted").and_then(serde_json::Value::as_bool).ok_or_else(invalid)
}

fn invalid() -> NetError {
    NetError::Proto(ProtoError::Protocol("Despres Connect discovery returned an invalid confirmation".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::{FakeHttp, json_answer};

    #[test]
    fn confirms_only_the_requested_domain_without_identity_or_credentials() {
        let server =
            FakeHttp::answering(json_answer("200 OK", r#"{"version":1,"provider":"despres-connect","domain":"tenant.example.org","hosted":true}"#));
        assert!(lookup_at(&server.url("/discovery"), "TENANT.EXAMPLE.ORG").unwrap());
        let request = server.request(0);
        assert!(request.starts_with("GET /discovery?domain=tenant.example.org HTTP/1.1\r\n"));
        assert!(request.ends_with("\r\n\r\n"));
        for forbidden in ["authorization", "cookie", "password", "@", "referer"] {
            assert!(!request.to_ascii_lowercase().contains(forbidden));
        }
        assert!(lookup_at(&server.url("/discovery"), "person@tenant.example.org").is_err());
    }

    #[test]
    fn missing_domains_errors_redirects_and_unbound_answers_never_confirm() {
        for (status, body) in
            [("404 Not Found", "{}"), ("200 OK", r#"{"version":1,"provider":"despres-connect","domain":"tenant.example.org","hosted":false}"#)]
        {
            let server = FakeHttp::answering(json_answer(status, body));
            assert!(!lookup_at(&server.url("/discovery"), "tenant.example.org").unwrap());
        }
        for (status, body) in [
            ("503 Unavailable", "private backend error"),
            ("302 Found", ""),
            ("200 OK", "not json"),
            ("200 OK", r#"{"version":1,"provider":"despres-connect","domain":"other.example.org","hosted":true}"#),
            ("200 OK", r#"{"version":2,"provider":"despres-connect","domain":"tenant.example.org","hosted":true}"#),
            ("200 OK", r#"{"version":1,"provider":"other","domain":"tenant.example.org","hosted":true}"#),
            ("200 OK", r#"{"version":1,"provider":"despres-connect","domain":"tenant.example.org","hosted":"true"}"#),
        ] {
            let server = FakeHttp::answering(json_answer(status, body));
            assert!(lookup_at(&server.url("/discovery"), "tenant.example.org").is_err());
        }
        let server = FakeHttp::answering(json_answer("200 OK", &"a".repeat(1025)));
        assert!(lookup_at(&server.url("/discovery"), "tenant.example.org").is_err());
    }
}
