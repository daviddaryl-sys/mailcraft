//! What a command keeps of the messages it brings content into and lets go of again (#173).

use std::sync::Mutex;

use serde_json::{Value, json};

use super::*;
use crate::{CmdResult, CommandSpec};

/// How many swaps the last `test.swapMany` held when it was done with its loads.
static HELD: Mutex<usize> = Mutex::new(0);

const PASSES: usize = 100;

/// Puts message `id` in place `PASSES` times over, as a search reading every body does, changes
/// message `other` for real, then ends as `how` says: `ok`, `fail` or `panic`.
fn swap_many(s: &mut Session, p: &Value) -> CmdResult {
    let id = p["id"].as_u64().unwrap();
    let other = p["other"].as_u64().unwrap();
    for i in 0..PASSES {
        let mut m = (**s.mailbox.messages.get(&id).unwrap()).clone();
        m.body.text = format!("pass {i}");
        s.put_message(id, m);
    }
    *HELD.lock().unwrap() = s.swaps.as_ref().map_or(0, Swaps::len);
    let mut o = (**s.mailbox.messages.get(&other).unwrap()).clone();
    o.read = !o.read;
    s.mailbox.messages.insert(other, Arc::new(o));
    match p["how"].as_str() {
        Some("ok") => Ok(json!({})),
        Some("fail") => Err(CmdError::Failed("asked to".into())),
        _ => panic!("asked to"),
    }
}

#[test]
fn putting_one_message_in_place_many_times_keeps_one_swap_and_a_failure_still_goes_back() {
    let now = DateTime::new(Date::new(2026, 10, 14).unwrap(), 16, 0, 0).unwrap().to_timestamp(0);
    let mut s = Session::at(crate::sample::mailbox(now, 0), now, 0);
    let mut specs = crate::cmd::registry().all().to_vec();
    specs.push(CommandSpec::new("test.swapMany", "Swap Many", "Test", swap_many));
    s.registry = Arc::new(Registry::new(specs));
    let mut ids = s.mailbox.messages.keys().copied();
    let (id, other) = (ids.next().unwrap(), ids.next().unwrap());
    let last = format!("pass {}", PASSES - 1);

    for how in ["fail", "panic"] {
        let read = s.mailbox.message(other).unwrap().read;
        assert!(s.run("test.swapMany", &json!({"id": id, "other": other, "how": how})).is_err());
        assert_eq!(*HELD.lock().unwrap(), 1, "{how}");
        // The real change is gone; the content brought in stays, as it was last put.
        assert_eq!(s.mailbox.message(other).unwrap().read, read, "{how}");
        assert_eq!(s.mailbox.message(id).unwrap().body.text, last, "{how}");
        assert!(!s.can_undo(), "{how}");
        assert!(!s.dirty, "{how}");
    }

    let read = s.mailbox.message(other).unwrap().read;
    s.run("test.swapMany", &json!({"id": id, "other": other, "how": "ok"})).unwrap();
    assert_eq!(*HELD.lock().unwrap(), 1);
    assert_ne!(s.mailbox.message(other).unwrap().read, read);
    // Undo takes back the change and keeps the content as the command left it.
    assert!(s.undo());
    assert_eq!(s.mailbox.message(other).unwrap().read, read);
    assert_eq!(s.mailbox.message(id).unwrap().body.text, last);
}
