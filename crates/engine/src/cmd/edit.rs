//! Undo and redo. For a Microsoft account, undoing a change that reached its server is another
//! change, which the next run sends; what can't be undone there is said ([`crate::undo`]).

use serde_json::{Value, json};

use crate::{CmdResult, CommandSpec, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("edit.undo", "Undo", "Quick Access", undo).key("Mod+Z").when(|s| if s.can_undo() { None } else { Some("nothing to undo") }),
        CommandSpec::new("edit.redo", "Redo", "Quick Access", redo)
            .key("Mod+Y / Mod+Shift+Z")
            .when(|s| if s.can_redo() { None } else { Some("nothing to redo") }),
    ]
}

fn undo(s: &mut Session, _: &Value) -> CmdResult {
    let label = s.undo_label();
    s.undo();
    let said = said(s);
    Ok(json!({"undone": label, "notUndone": said}))
}

fn redo(s: &mut Session, _: &Value) -> CmdResult {
    let label = s.redo_label();
    s.redo();
    let said = said(s);
    Ok(json!({"redone": label, "notRedone": said}))
}

/// What the undo or redo couldn't do, said in the status bar too.
fn said(s: &mut Session) -> Vec<String> {
    let said = s.undo_said().to_vec();
    if !said.is_empty() {
        s.sync_status = said.join(" ");
    }
    said
}
