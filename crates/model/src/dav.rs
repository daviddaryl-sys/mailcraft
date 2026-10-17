//! DAV sync checkpoints. Credentials never enter this persistent model.
use crate::{Contact, Event, Id, Mailbox};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Collection {
    pub account: Id,
    pub href: String,
    pub name: String,
    pub contacts: bool,
    pub read_only: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Resource {
    pub account: Id,
    pub collection: String,
    pub href: String,
    pub etag: String,
    /// Original document, including properties the UI does not understand.
    pub raw: String,
    pub contacts: Vec<Contact>,
    pub events: Vec<Event>,
}

/// A background job owns a snapshot. Deliberately has no Debug/Serialize implementation.
pub struct Request {
    pub account: Id,
    pub password: String,
    pub mailbox: Mailbox,
    /// Verification can discover/download without performing writes.
    pub read_only: bool,
}

#[derive(Default)]
pub struct Download {
    pub account: Id,
    pub url: String,
    pub before: Vec<Resource>,
    pub snapshots: Vec<Resource>,
    pub written: Vec<String>,
    pub resources: Vec<Resource>,
    pub removed: Vec<String>,
    pub collections: Vec<Collection>,
    pub errors: Vec<String>,
    pub complete: bool,
    pub uploaded: usize,
}
