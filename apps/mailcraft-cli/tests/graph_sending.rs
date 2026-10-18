//! Sending through Microsoft Graph (R3.4.2), from end to end against the strict Graph stand-in:
//! a message never saved goes with `sendMail`, a saved draft with send-draft after it is brought
//! up to date; the server's copy in Sent Items comes down and nothing is uploaded; a send whose
//! answer was lost, or that the server refused, is held and not sent again by itself. Nothing
//! here leaves this machine; every message is invented.
#![cfg(not(target_arch = "wasm32"))]

use std::sync::atomic::AtomicBool;

use mailcraft_engine::Session;
use mailcraft_engine::cmd::graph_mail::{apply_graph_mail, graph_mail_request};
use mailcraft_model::{FolderKind, Id, Mailbox};
use mailcraft_net::graph::standin::{StandIn, TOKEN};
use serde_json::{Value, json};

/// Friday 2027-01-15 08:00 UTC.
const NOW: i64 = 1_800_000_000;

struct Rig {
    server: StandIn,
    s: Session,
    account: Id,
}

impl Rig {
    fn new(address: &str) -> Rig {
        let server = StandIn::start().unwrap();
        let mut s = Session::at(Mailbox::with_local_account("", ""), NOW, 0);
        let account = s.run("account.add", &json!({"email": address, "kind": "microsoft"})).unwrap()["id"].as_u64().unwrap();
        let mut rig = Rig { server, s, account };
        rig.run();
        rig
    }

    fn run(&mut self) -> Vec<Value> {
        let request = graph_mail_request(&self.s, self.account).unwrap();
        let _routed = self.server.route();
        let cancel = AtomicBool::new(false);
        let s = &mut self.s;
        let mut answers = Vec::new();
        mailcraft_net::graph::sync_mail(TOKEN, &request, &cancel, &mut |piece| answers.push(apply_graph_mail(s, piece).unwrap()));
        answers
    }

    fn in_folder(&self, kind: FolderKind) -> Vec<mailcraft_model::Message> {
        let folder = self.s.mailbox.folder_of_kind(self.account, kind).unwrap();
        self.s.mailbox.messages_in(folder).map(|m| m.as_ref().clone()).collect()
    }

    fn compose(&mut self, subject: &str) {
        self.s.run("mail.new", &json!({"account": self.account, "to": "kit@example.org", "subject": subject, "body": "Hello."})).unwrap();
    }

    fn sent_there(&self) -> Vec<(String, String)> {
        self.server.state().sent.iter().map(|s| (s.subject.clone(), s.how.clone())).collect()
    }
}

#[test]
fn a_message_never_saved_goes_with_send_mail_and_its_copy_comes_down() {
    let mut rig = Rig::new("send-mail@example.org");
    rig.compose("Lunch?");
    rig.s.run("compose.send", &json!({})).unwrap();
    assert_eq!(rig.in_folder(FolderKind::Outbox).len(), 1);
    rig.run();
    assert_eq!(rig.sent_there(), [("Lunch?".to_string(), "sendMail".to_string())]);
    assert!(
        rig.server.state().requests.iter().any(|r| r.target == "/v1.0/me/sendMail" && r.header("content-type").starts_with("text/plain")),
        "as MIME"
    );
    assert!(rig.in_folder(FolderKind::Outbox).is_empty());
    // The server's copy in Sent Items, by delta; nothing was uploaded of it.
    let sent = rig.in_folder(FolderKind::Sent);
    assert_eq!(sent.len(), 1);
    assert!(sent[0].server.is_some() && sent[0].subject == "Lunch?");
    assert!(!rig.server.state().requests.iter().any(|r| r.method == "POST" && r.target.starts_with("/v1.0/me/messages")));
    rig.run();
    assert_eq!(rig.sent_there().len(), 1, "once");
    assert_eq!(rig.in_folder(FolderKind::Sent).len(), 1);
}

#[test]
fn a_saved_draft_is_brought_up_to_date_and_sent_as_that_draft() {
    let mut rig = Rig::new("send-draft@example.org");
    rig.compose("Report");
    let draft = rig.s.run("compose.saveDraft", &json!({})).unwrap()["draft"].as_u64().unwrap();
    rig.run();
    let drafts = rig.server.state().well_known("drafts");
    assert_eq!(rig.server.state().mailbox.counts(&drafts).0, 1);
    rig.s.run("mail.editDraft", &json!({"id": draft})).unwrap();
    rig.s.run("compose.set", &json!({"subject": "Report, final"})).unwrap();
    rig.s.run("compose.send", &json!({})).unwrap();
    assert!(rig.s.mailbox.message(draft).is_none(), "the draft became the message being sent");
    rig.run();
    assert_eq!(rig.sent_there(), [("Report, final".to_string(), "send".to_string())], "the change went first, then the draft");
    assert_eq!(rig.server.state().mailbox.counts(&drafts).0, 0, "not left in Drafts, not deleted from it either");
    assert!(!rig.server.state().requests.iter().any(|r| r.method == "DELETE"));
    assert!(!rig.server.state().requests.iter().any(|r| r.target == "/v1.0/me/sendMail"));
    rig.run();
    assert!(rig.in_folder(FolderKind::Outbox).is_empty() && rig.in_folder(FolderKind::Drafts).is_empty());
    assert_eq!(rig.in_folder(FolderKind::Sent).len(), 1);
    assert_eq!(rig.sent_there().len(), 1);
}

#[test]
fn a_send_whose_answer_is_lost_is_held_and_not_sent_again() {
    let mut rig = Rig::new("send-lost@example.org");
    rig.compose("Contract");
    rig.s.run("compose.send", &json!({})).unwrap();
    rig.server.state().lose_next.push("POST /v1.0/me/sendMail".into());
    rig.run();
    assert_eq!(rig.sent_there().len(), 1, "it went, and nobody here knows");
    let outbox = rig.in_folder(FolderKind::Outbox);
    assert_eq!(outbox.len(), 1);
    assert!(outbox[0].held.contains("may have been sent"), "{}", outbox[0].held);
    assert!(rig.s.sync_status.contains("may have been sent"), "{}", rig.s.sync_status);
    for _ in 0..3 {
        rig.run();
    }
    assert_eq!(rig.sent_there().len(), 1, "not sent again by itself");
    // The owner looked, and says send it again.
    let id = outbox[0].id;
    rig.s.run("mail.sendAgain", &json!({"id": id})).unwrap();
    rig.run();
    assert_eq!(rig.sent_there().len(), 2);
    assert!(rig.in_folder(FolderKind::Outbox).is_empty());
    assert!(rig.s.run("mail.sendAgain", &json!({"id": id})).is_err());
}

#[test]
fn a_send_the_server_refuses_is_held_with_why_and_a_lost_draft_send_too() {
    let mut rig = Rig::new("send-refused@example.org");
    rig.compose("Refused");
    rig.s.run("compose.send", &json!({})).unwrap();
    rig.server.state().fail_next.push(("POST /v1.0/me/sendMail".into(), 400, "ErrorInvalidRequest".into()));
    rig.run();
    let outbox = rig.in_folder(FolderKind::Outbox);
    assert!(outbox[0].held.starts_with("It wasn't sent"), "{}", outbox[0].held);
    rig.run();
    assert!(rig.sent_there().is_empty());

    rig.compose("Draft lost");
    rig.s.run("compose.saveDraft", &json!({})).unwrap();
    rig.run();
    let draft = rig.in_folder(FolderKind::Drafts)[0].id;
    rig.s.run("mail.editDraft", &json!({"id": draft})).unwrap();
    rig.s.run("compose.send", &json!({})).unwrap();
    rig.server.state().lose_next.push("POST /v1.0/me/messages/*/send".into());
    rig.run();
    rig.run();
    assert_eq!(rig.sent_there(), [("Draft lost".to_string(), "send".to_string())]);
    // Still held here, though its draft is gone from the server: it is the owner's to look into.
    assert!(rig.in_folder(FolderKind::Outbox).iter().any(|m| m.subject == "Draft lost" && m.held.contains("may have been sent")));
}
