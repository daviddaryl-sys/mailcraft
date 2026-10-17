//! Preserve unknown properties and components when editing a downloaded document.
use std::collections::BTreeMap;

fn logical(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        if let Some(tail) = line.strip_prefix([' ', '\t'])
            && let Some(last) = lines.last_mut()
        {
            last.push_str(tail);
        } else {
            lines.push(line.trim_end_matches('\r').into());
        }
    }
    lines
}
fn key(line: &str) -> String {
    line.split([':', ';']).next().unwrap_or("").rsplit('.').next().unwrap_or("").to_ascii_uppercase()
}
/// The direct properties and subcomponents of one VEVENT/VCARD, with all folded bytes unfolded.
fn groups(text: &str, component: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut depth = 0usize;
    let mut found = 0usize;
    let mut block = String::new();
    for line in logical(text) {
        let upper = line.to_ascii_uppercase();
        if upper == format!("BEGIN:{component}") {
            found += 1;
            depth = 1;
            continue;
        }
        if depth == 0 {
            continue;
        }
        if upper.starts_with("BEGIN:") {
            if depth == 1 {
                block = upper.clone();
            }
            depth += 1;
        }
        if upper.starts_with("END:") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                continue;
            }
        }
        let k = if depth > 1 || upper.starts_with("END:") { block.clone() } else { key(&line) };
        out.entry(k).or_default().push(line);
    }
    if found != 1 || depth != 0 {
        return Err("Editing a DAV resource with multiple components is not supported; edit it in Despres Connect".into());
    }
    Ok(out)
}
pub fn patch(original: &str, before: &str, after: &str, component: &str) -> Result<String, String> {
    if component == "VCARD" {
        return patch_card(original, before, after);
    }
    let old = groups(before, component)?;
    let new = groups(after, component)?;
    let changed: Vec<String> = old.keys().chain(new.keys()).filter(|k| k.as_str() != "UID" && old.get(*k) != new.get(*k)).cloned().collect();
    if changed.is_empty() {
        return Ok(original.into());
    }
    // A detached recurrence shares its resource with its master. Never drop either.
    let _ = groups(original, component)?;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut block = String::new();
    for line in logical(original) {
        let upper = line.to_ascii_uppercase();
        if upper == format!("BEGIN:{component}") {
            depth = 1;
            out.push(line);
            for (k, lines) in &new {
                if changed.contains(k) {
                    out.extend(lines.clone());
                }
            }
            continue;
        }
        if depth == 0 {
            out.push(line);
            continue;
        }
        if upper.starts_with("BEGIN:") {
            if depth == 1 {
                block = upper.clone();
            }
            depth += 1;
        }
        if upper.starts_with("END:") {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                out.push(line);
                continue;
            }
        }
        let k = if depth > 1 || upper.starts_with("END:") { block.clone() } else { key(&line) };
        if !changed.contains(&k) {
            out.push(line);
        }
    }
    // Keep original timezone definitions. Refuse introducing a zone which has no definition.
    if component == "VEVENT" && changed.iter().any(|k| k == "DTSTART" || k == "DTEND") {
        for line in logical(after).iter().filter(|l| l.starts_with("DTSTART;TZID=") || l.starts_with("DTEND;TZID=")) {
            if let Some(tz) = line.split_once("TZID=").and_then(|(_, v)| v.split(':').next())
                && !original.contains(&format!("TZID:{tz}"))
                && !original.contains(&format!("TZID={tz}:"))
            {
                return Err("Changing an event to a new time zone requires Despres Connect".into());
            }
        }
    }
    Ok(fold(&out))
}
fn fold(lines: &[String]) -> String {
    let mut out = String::new();
    for line in lines {
        let mut count = 0;
        for c in line.chars() {
            if count + c.len_utf8() > 75 {
                out.push_str("\r\n ");
                count = 1;
            }
            out.push(c);
            count += c.len_utf8();
        }
        out.push_str("\r\n");
    }
    out
}
fn group(line: &str) -> Option<&str> {
    line.split([':', ';']).next()?.split_once('.').map(|(prefix, _)| prefix)
}
fn patch_card(original: &str, before: &str, after: &str) -> Result<String, String> {
    let old = groups(before, "VCARD")?;
    let new = groups(after, "VCARD")?;
    let _ = groups(original, "VCARD")?;
    let mut changed: Vec<_> =
        old.keys().chain(new.keys()).filter(|k| !matches!(k.as_str(), "UID" | "X-ABLABEL") && old.get(*k) != new.get(*k)).cloned().collect();
    if old.get("X-ABLABEL") != new.get("X-ABLABEL") {
        for property in ["TEL", "EMAIL", "ADR", "URL"] {
            if old.get(property).into_iter().chain(new.get(property)).flatten().any(|line| group(line).is_some()) {
                changed.push(property.into());
            }
        }
    }
    if changed.is_empty() {
        return Ok(original.into());
    }
    let source = logical(original);
    let target = logical(after);
    let affected = |lines: &[String]| -> Vec<String> {
        lines.iter().filter(|l| changed.contains(&key(l))).filter_map(|l| group(l).map(str::to_string)).collect()
    };
    let old_groups = affected(&source);
    let new_groups = affected(&target);
    // Generated group labels must not collide with photos or other untouched source groups.
    let mut number = 1;
    let prefix = loop {
        let p = format!("mailcraft{number}-");
        if !source.iter().any(|l| l.starts_with(&p)) {
            break p;
        }
        number += 1;
    };
    let mut added = Vec::new();
    for line in &target {
        let label = key(line) == "X-ABLABEL" && group(line).is_some_and(|g| new_groups.iter().any(|v| v == g));
        if changed.contains(&key(line)) || label {
            added.push(if let Some(g) = group(line) { format!("{prefix}{g}.{}", line.split_once('.').map_or("", |(_, v)| v)) } else { line.clone() });
        }
    }
    let mut out = Vec::new();
    for line in source {
        if line.eq_ignore_ascii_case("END:VCARD") {
            out.extend(added.clone());
        }
        let label = key(&line) == "X-ABLABEL" && group(&line).is_some_and(|g| old_groups.iter().any(|v| v == g));
        if !changed.contains(&key(&line)) && !label {
            out.push(line);
        }
    }
    Ok(fold(&out))
}
pub fn meeting(text: &str) -> bool {
    logical(text).iter().any(|line| matches!(key(line).as_str(), "ORGANIZER" | "ATTENDEE"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editing_keeps_photos_and_unknown_properties() {
        let original = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:keep\r\nFN:Old\r\nPHOTO;ENCODING=b:abc\r\n def\r\nX-PRIVATE:opaque\r\nEND:VCARD\r\n";
        let before = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:keep\r\nFN:Old\r\nEND:VCARD\r\n";
        let after = before.replace("FN:Old", "FN:New");
        let got = patch(original, before, &after, "VCARD").unwrap();
        assert!(got.contains("FN:New"));
        assert!(got.contains("PHOTO;ENCODING=b:abcdef"));
        assert!(got.contains("X-PRIVATE:opaque"));
        assert!(got.contains("UID:keep"));
    }
    #[test]
    fn refuses_multiple_event_resource_and_detects_scheduling() {
        let e = "BEGIN:VEVENT\r\nUID:a\r\nSUMMARY:Old\r\nEND:VEVENT\r\n";
        assert!(patch(&(e.to_owned() + e), e, &e.replace("Old", "New"), "VEVENT").is_err());
        assert!(meeting("ATTENDEE;CN=Somebody:mailto:a@example.test\r\n"));
    }
}

#[cfg(test)]
mod card_group_tests {
    use super::*;
    #[test]
    fn editing_a_phone_keeps_an_unrelated_photo_label() {
        let original = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:a\r\nFN:A\r\nitem1.PHOTO:data\r\nitem1.X-ABLabel:Portrait\r\nitem2.TEL:123\r\nitem2.X-ABLabel:Old label\r\nEND:VCARD\r\n";
        let before = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:a\r\nFN:A\r\nitem1.TEL:123\r\nitem1.X-ABLabel:Old label\r\nEND:VCARD\r\n";
        let after = before.replace("TEL:123", "TEL:456").replace("Old label", "New label");
        let got = patch(original, before, &after, "VCARD").unwrap();
        assert!(got.contains("item1.PHOTO:data"));
        assert!(got.contains("item1.X-ABLabel:Portrait"));
        assert!(!got.contains("Old label"));
        assert!(got.contains("mailcraft1-item1.TEL:456"));
        assert!(got.contains("mailcraft1-item1.X-ABLabel:New label"));
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;
    #[test]
    fn a_label_only_edit_is_not_silently_discarded() {
        let before = "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:a\r\nFN:A\r\nitem1.TEL:123\r\nitem1.X-ABLabel:Old label\r\nEND:VCARD\r\n";
        let after = before.replace("Old label", "New label");
        let got = patch(before, before, &after, "VCARD").unwrap();
        assert!(got.contains("New label"));
        assert!(!got.contains("Old label"));
        assert!(got.contains("TEL:123"));
    }
}
