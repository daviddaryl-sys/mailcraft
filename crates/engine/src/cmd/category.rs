//! Colour categories.

use mailcraft_model::Rgb;
use serde_json::{Value, json};

use super::{bad, done};
use crate::{CmdResult, CommandSpec, Session, p};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("category.set", "New Category", "Home › Tags", set).params(r##"{"name": string, "color": "#RRGGBB"}"##),
        CommandSpec::new("category.rename", "Rename Category", "Home › Tags", rename).params(r#"{"from": string, "to": string}"#),
        CommandSpec::new("category.remove", "Delete Category", "Home › Tags", remove).params(r#"{"name": string}"#),
        CommandSpec::new("category.list", "All Categories", "Home › Tags", list).pure(),
    ]
}

fn set(s: &mut Session, v: &Value) -> CmdResult {
    let color = Rgb::parse(p::req_str(v, "color")?).ok_or_else(|| bad("`color` looks like #2E8B57"))?;
    if s.mailbox.categories.len() >= 200 && s.mailbox.category(p::req_str(v, "name")?).is_none() {
        return Err(super::fail("there are too many categories"));
    }
    // A category on Outlook's list has Outlook's colour (R3 spec S18): nothing changes here or
    // on the server.
    let name = p::req_str(v, "name")?;
    if on_outlooks_list(s, name) && s.mailbox.category(name).is_some_and(|c| c.color != color) {
        return Err(super::fail(COLOUR_IN_OUTLOOK));
    }
    s.mailbox.set_category(name, color)?;
    done()
}

/// What changing the colour of a category on Outlook's list says.
pub const COLOUR_IN_OUTLOOK: &str = "The colour is set in Outlook";

/// Whether `name` is on Outlook's list of categories, as a Microsoft account's server last gave it.
pub fn on_outlooks_list(s: &Session, name: &str) -> bool {
    s.mailbox.outlook_categories.iter().any(|c| mailcraft_model::sync::same_category(c, name.trim()))
}

fn rename(s: &mut Session, v: &Value) -> CmdResult {
    s.mailbox.rename_category(p::req_str(v, "from")?, p::req_str(v, "to")?)?;
    done()
}

fn remove(s: &mut Session, v: &Value) -> CmdResult {
    s.mailbox.remove_category(p::req_str(v, "name")?)?;
    done()
}

fn list(s: &mut Session, _: &Value) -> CmdResult {
    // `outlook`: on Outlook's list (its colour is Outlook's); `null` while no such list was read.
    let read = !s.mailbox.outlook_categories.is_empty();
    let listed = |name: &str| read.then(|| on_outlooks_list(s, name));
    Ok(
        json!({"categories": s.mailbox.categories.iter().map(|c| json!({"name": c.name, "color": c.color.hex(), "outlook": listed(&c.name)})).collect::<Vec<_>>()}),
    )
}
