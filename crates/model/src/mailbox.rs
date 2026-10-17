//! The mailbox: everything MailCraft keeps, with the primitives that edit it.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::sync::RemoteFlags;
use crate::{Action, Calendar, Contact, Event, Id, Message, ModelError, Rgb, Rule, Signature, Task, clean_name};

/// Deepest a folder tree may nest (a hostile file could otherwise chain folders without end).
const MAX_DEPTH: usize = 32;
/// Largest id a file may hold: 2^53, the largest integer every JSON reader keeps exactly.
const MAX_ID: Id = 1 << 53;
/// Most changes waiting for the mail server that a file may hold ([`Mailbox::pending`]).
const MAX_PENDING: usize = 100_000;
/// Most folders one account's Favorites from its server hold ([`Mailbox::server_favorites`]).
const MAX_SERVER_FAVORITES: usize = crate::remote::MAX_FAVORITES;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderKind {
    Inbox,
    Drafts,
    Sent,
    Deleted,
    Junk,
    Archive,
    Outbox,
    #[default]
    Custom,
}

impl FolderKind {
    /// The folders every account has, in the order the folder pane lists them.
    pub const STANDARD: [FolderKind; 7] =
        [FolderKind::Inbox, FolderKind::Drafts, FolderKind::Sent, FolderKind::Deleted, FolderKind::Archive, FolderKind::Junk, FolderKind::Outbox];

    pub fn default_name(self) -> &'static str {
        match self {
            FolderKind::Inbox => "Inbox",
            FolderKind::Drafts => "Drafts",
            FolderKind::Sent => "Sent Items",
            FolderKind::Deleted => "Deleted Items",
            FolderKind::Junk => "Junk Email",
            FolderKind::Archive => "Archive",
            FolderKind::Outbox => "Outbox",
            FolderKind::Custom => "Folder",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            FolderKind::Inbox => "inbox",
            FolderKind::Drafts => "drafts",
            FolderKind::Sent => "sent",
            FolderKind::Deleted => "deleted",
            FolderKind::Junk => "junk",
            FolderKind::Archive => "archive",
            FolderKind::Outbox => "outbox",
            FolderKind::Custom => "custom",
        }
    }

    pub fn parse(s: &str) -> Option<FolderKind> {
        let s = s.trim().to_ascii_lowercase();
        Self::STANDARD.into_iter().chain([FolderKind::Custom]).find(|k| k.code() == s)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Folder {
    pub id: Id,
    pub account: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Id>,
    pub name: String,
    pub kind: FolderKind,
    pub favorite: bool,
    /// Its path on the server (IMAP), when it has one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub remote: String,
    /// IMAP `UIDVALIDITY` and the highest UID already fetched.
    #[serde(skip_serializing_if = "is_zero")]
    pub uid_validity: u32,
    #[serde(skip_serializing_if = "is_zero")]
    pub uid_seen: u32,
    /// For a folder of a [`AccountKind::Microsoft`] account that the server has: what the
    /// server calls it and what it last said of it ([`ServerFolder`]). `None` for a folder that
    /// only exists here (one made here that the server is yet to hear of, or a folder of any
    /// other account). A file written before this was kept has none.
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "crate::sync::lenient")]
    pub server: Option<ServerFolder>,
}

/// A folder as a Microsoft account's server knows it (ADR 0003): its server id, which stays the
/// same when the folder is renamed or moved, and what the server last said of its name and of
/// the folder it lies in. A change made here is the difference between those and the folder as
/// it is now; nothing else is kept of it.
///
/// The id is one mailbox's own: a folder of a shared mailbox (after beta) will also need that
/// mailbox's address, which goes beside the id here, not into it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerFolder {
    /// The server's id of the folder (Graph's immutable id).
    pub id: String,
    /// Its name, as the server last said.
    pub name: String,
    /// The server id of the folder it lies in, as the server last said; empty for one at the top
    /// of the mailbox.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub parent: String,
    /// Where the folder's mail goes on from next time (Graph's delta link, or the next page of a
    /// listing that was cut short); empty until its mail has been read once (R3.2).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub delta: String,
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Security {
    /// No encryption: only for a server on this machine.
    None,
    /// Upgrade a plain connection with STARTTLS.
    StartTls,
    /// TLS from the first byte.
    #[default]
    Tls,
}

impl Security {
    pub fn parse(s: &str) -> Option<Security> {
        match s.trim().to_ascii_lowercase().replace(['/', ' ', '-'], "").as_str() {
            "none" | "plain" => Some(Security::None),
            "starttls" => Some(Security::StartTls),
            "tls" | "ssl" | "ssltls" => Some(Security::Tls),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Security::None => "none",
            Security::StartTls => "starttls",
            Security::Tls => "tls",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub security: Security,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccountKind {
    /// Lives in this mailbox only.
    #[default]
    Local,
    /// Synchronised with an IMAP server; sends through SMTP.
    Imap,
    /// A Microsoft account (Outlook.com, Microsoft 365) whose folders and mail are kept in step
    /// through Microsoft Graph alone, never by IMAP or SMTP (ADR 0002). It has no server
    /// settings and signs in with Microsoft ([`Account::oauth`]). Until R3 is whole one is made
    /// only behind a switch for testing (`MAILCRAFT_GRAPH_MAIL=1`, R3 spec S35): without it
    /// every account is what it was.
    Microsoft,
}

impl AccountKind {
    /// Whether the account is kept in step with a server (by IMAP or by Microsoft Graph).
    pub fn has_server(self) -> bool {
        matches!(self, AccountKind::Imap | AccountKind::Microsoft)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Account {
    pub id: Id,
    /// What the folder pane calls it (usually the address).
    pub name: String,
    pub email: String,
    /// The owner's name, as recipients see it.
    pub display_name: String,
    pub kind: AccountKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incoming: Option<Server>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outgoing: Option<Server>,
    /// Login name; the password is never stored in the mailbox.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub username: String,
    /// Optional CalDAV/CardDAV discovery URL; uses this account's password.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dav_url: String,
    /// The signature new messages start with (a [`Signature`] name; empty for none).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub signature: String,
    /// The signature replies and forwards start with (empty for none); `None` for an account from
    /// before there were two, which has [`Account::signature`] for both ([`Account::reply_signature`]).
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "crate::sync::lenient")]
    pub reply_signature: Option<String>,
    /// Set when the owner signs in on the provider's own page (OAuth 2.0) instead of with a
    /// password. No token is ever stored here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oauth: Option<OAuth>,
    /// A mailbox written while the owner of an account could choose how its Favorites in
    /// Outlook were read (`favoritesSource`, 0.2.0 to 0.2.1) and had chosen anything but
    /// Microsoft Graph: Exchange Web Services, which is gone, or not now. That choice is no
    /// more: they are read through Graph, as for every account that signs in with Microsoft,
    /// and the owner is told so once ([`Mailbox::take_retired_favorites_choices`]). Read from a
    /// file, never written to one: the next save drops it.
    #[serde(rename = "favoritesSource", skip_serializing, deserialize_with = "retired_favorites_choice")]
    pub retired_favorites_choice: bool,
}

/// Whether what a mailbox file says where `favoritesSource` was is a choice that no longer
/// stands ([`Account::retired_favorites_choice`]): Exchange Web Services, or not now, in the
/// words those versions wrote and read. Whatever else is there, the file opens, and nothing is
/// said of it.
fn retired_favorites_choice<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    let said: Option<String> = crate::sync::lenient(deserializer)?;
    Ok(said.is_some_and(|way| matches!(way.trim().to_ascii_lowercase().as_str(), "ews" | "none" | "notnow" | "own")))
}

/// Which kind of Microsoft account signs in: one of an organisation (work or school), or a
/// personal one (Outlook.com). The sign-in itself says which, by the directory it names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Directory {
    /// Nobody has said yet.
    #[default]
    Unknown,
    /// A work or school account: its organisation's own directory.
    Organisation,
    /// A personal Microsoft account.
    Personal,
}

impl Directory {
    /// The directory every personal Microsoft account is in, as an identity token names it
    /// (its `tid` claim; Microsoft identity platform, "ID token claims reference").
    pub const PERSONAL_TENANT: &'static str = "9188040d-6c67-4c5b-b112-36a304b66dad";

    pub fn is_unknown(&self) -> bool {
        *self == Directory::Unknown
    }

    pub fn parse(s: &str) -> Option<Directory> {
        match s.trim().to_ascii_lowercase().as_str() {
            "organisation" | "organization" | "work" => Some(Directory::Organisation),
            "personal" | "consumer" => Some(Directory::Personal),
            _ => None,
        }
    }

    /// What a command calls it; `None` when it isn't known.
    pub fn code(self) -> Option<&'static str> {
        match self {
            Directory::Unknown => None,
            Directory::Organisation => Some("organisation"),
            Directory::Personal => Some("personal"),
        }
    }

    /// The kind of account that signed in to the directory `tenant`: what an identity token
    /// names (a GUID), or what the sign-in settings name (`consumers`, `organizations`, a
    /// directory's id or domain). `common` and nothing say nothing.
    pub fn of_tenant(tenant: &str) -> Directory {
        match tenant.trim().to_ascii_lowercase().as_str() {
            "" | "common" => Directory::Unknown,
            "consumers" | Directory::PERSONAL_TENANT => Directory::Personal,
            _ => Directory::Organisation,
        }
    }
}

impl Account {
    /// The name of the signature replies and forwards start with (empty for none).
    pub fn reply_signature(&self) -> &str {
        self.reply_signature.as_deref().unwrap_or(&self.signature)
    }

    pub fn address(&self) -> crate::Address {
        crate::Address::new(self.display_name.clone(), self.email.clone())
    }

    /// The name the servers know the owner by: the login name, or else the address.
    pub fn login(&self) -> &str {
        let user = self.username.trim();
        if user.is_empty() { self.email.trim() } else { user }
    }
}

/// Who signs the owner in when it isn't a password.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OAuthProvider {
    /// Outlook.com and Microsoft 365.
    #[default]
    Microsoft,
}

impl OAuthProvider {
    pub fn parse(s: &str) -> Option<OAuthProvider> {
        match s.trim().to_ascii_lowercase().as_str() {
            "microsoft" | "outlook" | "office365" | "microsoft365" => Some(OAuthProvider::Microsoft),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => "microsoft",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            OAuthProvider::Microsoft => "Microsoft",
        }
    }
}

/// OAuth 2.0 sign-in: the mail servers take a short-lived token (SASL XOAUTH2) instead of a
/// password. Plain settings only; the tokens live in memory with whoever holds the sockets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OAuth {
    pub provider: OAuthProvider,
    /// The application (client) id presented to the provider; empty means the build's own.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub client_id: String,
    /// Microsoft: `common`, `consumers`, `organizations` or a directory id; empty means `common`.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub tenant: String,
    /// Microsoft: whether the account that signs in is an organisation's or a personal one, when
    /// that is known ([`Directory`]). Written when the account is set up, and not otherwise;
    /// never the directory's id.
    #[serde(skip_serializing_if = "Directory::is_unknown", deserialize_with = "crate::sync::lenient_or_default")]
    pub directory: Directory,
}

/// A change made here to what a Microsoft account's server has that comparing with what the
/// server last said can't show (ADR 0003): it waits here until a run has told the server. Each
/// names what it is about by its server id. (R3.3 adds a copy and a delete for good of a
/// message.)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Queued {
    /// A folder was deleted for good here (from Deleted Items, or by emptying it): the server
    /// deletes it too, which keeps it recoverable for the server's retention period.
    DeleteFolder { account: Id, id: String },
}

impl Queued {
    pub fn account(&self) -> Id {
        match self {
            Queued::DeleteFolder { account, .. } => *account,
        }
    }
}

/// A change made here to a message the mail server has, which the server is yet to be told. The
/// next synchronisation pushes it, and it stays listed until the server has taken it on
/// ([`crate::sync::RemoteOp`]). The server's copy is named by the path of its mailbox (`from`,
/// the [`Folder::remote`] of a folder of `account`) and its UID there.
///
/// Flag changes aren't listed: they show as a message whose flags differ from its
/// [`Message::synced`]. Nor is anything done to a folder: removing or renaming one is not
/// pushed to the server, and removing a folder or an account drops the changes that mention it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Pending {
    /// A message that was `uid` in server mailbox `from` now belongs in local folder `to`.
    /// `synced` is what its [`Message::synced`] was when it left, for when it has a UID again.
    Move {
        account: Id,
        message: Id,
        from: String,
        uid: u32,
        #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::sync::lenient")]
        synced: Option<RemoteFlags>,
        to: Id,
    },
    /// The message that was `uid` in server mailbox `from` was deleted for good. `message_id`
    /// is its Message-ID, by which it is found again when the server numbers the mailbox anew
    /// (`uid` is 0 until then).
    Delete {
        account: Id,
        from: String,
        uid: u32,
        #[serde(default, rename = "messageId", skip_serializing_if = "String::is_empty")]
        message_id: String,
    },
}

impl Pending {
    pub fn account(&self) -> Id {
        match self {
            Pending::Move { account, .. } | Pending::Delete { account, .. } => *account,
        }
    }

    /// The server's copy the change is about: its mailbox's path and its UID there.
    pub fn source(&self) -> (&str, u32) {
        match self {
            Pending::Move { from, uid, .. } | Pending::Delete { from, uid, .. } => (from, *uid),
        }
    }
}

/// The Favorites of an account as its server lists them (the folders Outlook shows under
/// Favorites): the mailbox's own folders for them, in the server's order. While an account has
/// such a list, the folder pane can show it in place of the favourites that are marked here
/// ([`Folder::favorite`]), which stay as they are. Only ever read from the server, never sent.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerFavorites {
    pub account: Id,
    /// Folders of that account, in the server's order. May be empty: the server has a list, and
    /// nothing in it is a folder of this mailbox.
    pub folders: Vec<Id>,
}

/// A colour category (`Red category`), applied to messages, events, contacts and tasks by name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Category {
    pub name: String,
    pub color: Rgb,
}

/// The categories a new mailbox starts with (our own colours).
fn default_categories() -> Vec<Category> {
    [
        ("Red category", Rgb(0xD1, 0x3B, 0x3B)),
        ("Orange category", Rgb(0xE0, 0x7B, 0x1F)),
        ("Yellow category", Rgb(0xD9, 0xA4, 0x0C)),
        ("Green category", Rgb(0x2E, 0x8B, 0x57)),
        ("Blue category", Rgb(0x2F, 0x6F, 0xD0)),
        ("Purple category", Rgb(0x7E, 0x57, 0xC2)),
    ]
    .into_iter()
    .map(|(name, color)| Category { name: name.into(), color })
    .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Mailbox {
    /// File format version.
    pub version: u32,
    pub accounts: Vec<Account>,
    pub folders: Vec<Folder>,
    pub messages: BTreeMap<Id, Arc<Message>>,
    pub contacts: BTreeMap<Id, Arc<Contact>>,
    /// DAV ETags and lossless server documents used for conditional two-way sync.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dav_resources: Vec<crate::dav::Resource>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dav_collections: Vec<crate::dav::Collection>,
    pub calendars: Vec<Calendar>,
    pub events: BTreeMap<Id, Arc<Event>>,
    pub tasks: BTreeMap<Id, Arc<Task>>,
    pub categories: Vec<Category>,
    pub rules: Vec<Rule>,
    pub signatures: Vec<Signature>,
    /// Moves and deletions the mail server is yet to be told, oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "crate::sync::lenient_list")]
    pub pending: Vec<Pending>,
    /// Changes made here to events of calendars that a server keeps, which that server is yet
    /// to be told, oldest first ([`crate::remote::EventChange`]).
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "crate::sync::lenient_list")]
    pub event_changes: Vec<crate::remote::EventChange>,
    /// Changes to events that a server said it took and that no download has shown yet
    /// ([`crate::remote::EventCheck`]).
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "crate::sync::lenient_list")]
    pub event_checks: Vec<crate::remote::EventCheck>,
    /// The Favorites of the accounts whose server has a list of them ([`ServerFavorites`]), one
    /// entry an account at most ([`Mailbox::server_favorites`]). Nothing is written of it until
    /// a server's list has been taken in.
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "crate::sync::lenient_list")]
    pub favorites_from_server: Vec<ServerFavorites>,
    /// Changes to what a Microsoft account's server has that its next run is to tell it and that
    /// can't be worked out by comparing ([`Queued`]), oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "crate::sync::lenient_list")]
    pub queued: Vec<Queued>,
    /// The time zone the wall-clock times of the events are written in ([`Event::start`]): the
    /// zone the calendar was shown in when they were written, by its name in the IANA database
    /// (`Europe/Berlin`) or as an offset (`UTC+05:30`). Empty in a mailbox written before zones
    /// were kept, and when the system couldn't say which zone it is in: the times are then
    /// taken as this computer's, as they always were. Whoever shows the calendar in another
    /// zone has the times rewritten first ([`Mailbox::rebase`]).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub zone: String,
    next_id: Id,
}

impl Default for Mailbox {
    fn default() -> Self {
        Mailbox {
            version: 1,
            accounts: Vec::new(),
            folders: Vec::new(),
            messages: BTreeMap::new(),
            contacts: BTreeMap::new(),
            dav_resources: Vec::new(),
            dav_collections: Vec::new(),
            calendars: Vec::new(),
            events: BTreeMap::new(),
            tasks: BTreeMap::new(),
            categories: default_categories(),
            rules: Vec::new(),
            signatures: Vec::new(),
            pending: Vec::new(),
            event_changes: Vec::new(),
            event_checks: Vec::new(),
            favorites_from_server: Vec::new(),
            queued: Vec::new(),
            zone: String::new(),
            next_id: 1,
        }
    }
}

fn not_found(what: &'static str, id: Id) -> ModelError {
    ModelError::NotFound(what, id)
}

fn invalid(msg: impl Into<String>) -> ModelError {
    ModelError::Invalid(msg.into())
}

impl Mailbox {
    /// An empty mailbox with one local account, its standard folders and a calendar.
    pub fn with_local_account(display_name: &str, email: &str) -> Mailbox {
        let mut mb = Mailbox::default();
        mb.add_account(Account { name: email.to_string(), email: email.to_string(), display_name: display_name.to_string(), ..Default::default() });
        mb
    }

    /// Reserve an identity when incorporating already assigned server items into a snapshot.
    pub fn reserve_id(&mut self, id: Id) {
        if id <= MAX_ID {
            self.next_id = self.next_id.max(id.saturating_add(1));
        }
    }

    pub fn alloc_id(&mut self) -> Id {
        let id = self.next_id.max(1);
        self.next_id = id.saturating_add(1);
        id
    }

    // ---- accounts ----

    /// Add an account with its standard folders; the first account also gets the default calendar.
    pub fn add_account(&mut self, mut account: Account) -> Id {
        account.id = self.alloc_id();
        let id = account.id;
        if account.name.trim().is_empty() {
            account.name = if account.email.trim().is_empty() { "Mailbox".into() } else { account.email.clone() };
        }
        self.accounts.push(account);
        for kind in FolderKind::STANDARD {
            let fid = self.alloc_id();
            self.folders.push(Folder {
                id: fid,
                account: id,
                name: kind.default_name().into(),
                kind,
                favorite: kind == FolderKind::Inbox,
                ..Default::default()
            });
        }
        if self.calendars.is_empty() {
            let cid = self.alloc_id();
            self.calendars.push(Calendar { id: cid, name: "Calendar".into(), color: Rgb(0x2F, 0x6F, 0xD0), visible: true, ..Default::default() });
        }
        id
    }

    pub fn account(&self, id: Id) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    pub fn account_mut(&mut self, id: Id) -> Result<&mut Account, ModelError> {
        self.accounts.iter_mut().find(|a| a.id == id).ok_or(not_found("account", id))
    }

    /// The account new mail is written from: the first one.
    pub fn default_account(&self) -> Option<&Account> {
        self.accounts.first()
    }

    /// Remove an account with its folders and messages. Its server isn't told anything: what
    /// was still waiting to be pushed there is dropped.
    pub fn remove_account(&mut self, id: Id) -> Result<(), ModelError> {
        let before = self.accounts.len();
        self.accounts.retain(|a| a.id != id);
        if self.accounts.len() == before {
            return Err(not_found("account", id));
        }
        self.folders.retain(|f| f.account != id);
        self.messages.retain(|_, m| m.account != id);
        self.pending.retain(|p| p.account() != id);
        self.event_changes.retain(|change| change.account != id);
        self.event_checks.retain(|check| check.account != id);
        self.queued.retain(|queued| queued.account() != id);
        self.favorites_from_server.retain(|list| list.account != id);
        self.dav_resources.retain(|r| r.account != id);
        self.dav_collections.retain(|c| c.account != id);
        Ok(())
    }

    // ---- favourites ----

    /// The folders of `account` that its server lists as Favorites, in the server's order;
    /// `None` when the mailbox has no list from that account's server. Only folders that the
    /// account has are named, each once (a file may say anything).
    pub fn server_favorites(&self, account: Id) -> Option<Vec<Id>> {
        let list = self.favorites_from_server.iter().find(|list| list.account == account)?;
        let mut folders: Vec<Id> = Vec::new();
        for id in list.folders.iter().take(MAX_SERVER_FAVORITES) {
            if !folders.contains(id) && self.folder(*id).is_some_and(|f| f.account == account) {
                folders.push(*id);
            }
        }
        Some(folders)
    }

    /// Keep the Favorites of `account` as its server lists them (`None`: its server has no list,
    /// and what was kept of one goes). The favourites marked here are not touched. Whether
    /// anything is different for it.
    pub fn set_server_favorites(&mut self, account: Id, folders: Option<Vec<Id>>) -> bool {
        let at = self.favorites_from_server.iter().position(|list| list.account == account);
        match (folders, at) {
            (None, None) => false,
            (None, Some(_)) => {
                self.favorites_from_server.retain(|list| list.account != account);
                true
            }
            (Some(mut folders), at) => {
                if self.account(account).is_none() {
                    return false;
                }
                let mut seen: Vec<Id> = Vec::new();
                folders.retain(|id| {
                    let fits = seen.len() < MAX_SERVER_FAVORITES && !seen.contains(id) && self.folder(*id).is_some_and(|f| f.account == account);
                    if fits {
                        seen.push(*id);
                    }
                    fits
                });
                match at.and_then(|at| self.favorites_from_server.get_mut(at)) {
                    Some(list) if list.folders == folders => false,
                    Some(list) => {
                        list.folders = folders;
                        true
                    }
                    None => {
                        // (One list an account: what a file had twice goes with the first.)
                        self.favorites_from_server.push(ServerFavorites { account, folders });
                        true
                    }
                }
            }
        }
    }

    // ---- folders ----

    pub fn folder(&self, id: Id) -> Option<&Folder> {
        self.folders.iter().find(|f| f.id == id)
    }

    pub fn folder_mut(&mut self, id: Id) -> Result<&mut Folder, ModelError> {
        self.folders.iter_mut().find(|f| f.id == id).ok_or(not_found("folder", id))
    }

    /// An account's standard folder of `kind`.
    pub fn folder_of_kind(&self, account: Id, kind: FolderKind) -> Option<Id> {
        self.folders.iter().find(|f| f.account == account && f.kind == kind && f.parent.is_none()).map(|f| f.id)
    }

    /// The default account's Inbox (or any folder when the mailbox has no Inbox).
    pub fn inbox(&self) -> Option<Id> {
        self.default_account().and_then(|a| self.folder_of_kind(a.id, FolderKind::Inbox)).or_else(|| self.folders.first().map(|f| f.id))
    }

    /// Top-level folders of an account (`parent` `None`) or a folder's subfolders, standard
    /// folders first in their usual order, then the others by name.
    pub fn children(&self, account: Id, parent: Option<Id>) -> Vec<&Folder> {
        let mut v: Vec<&Folder> = self.folders.iter().filter(|f| f.account == account && f.parent == parent).collect();
        let rank = |f: &Folder| FolderKind::STANDARD.iter().position(|k| *k == f.kind).unwrap_or(usize::MAX);
        v.sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        v
    }

    /// A folder and everything under it.
    pub fn subtree(&self, id: Id) -> Vec<Id> {
        let mut out = vec![id];
        let mut i = 0;
        while let Some(cur) = out.get(i).copied() {
            i += 1;
            if out.len() > self.folders.len() {
                break;
            }
            out.extend(self.folders.iter().filter(|f| f.parent == Some(cur) && !out.contains(&f.id)).map(|f| f.id).collect::<Vec<_>>());
        }
        out
    }

    /// `Inbox/Projects/Apollo`.
    pub fn folder_path(&self, id: Id) -> String {
        let mut parts = Vec::new();
        let mut cur = Some(id);
        while let Some(f) = cur.and_then(|c| self.folder(c)) {
            parts.push(f.name.as_str());
            cur = f.parent;
            if parts.len() > MAX_DEPTH {
                break;
            }
        }
        parts.reverse();
        parts.join("/")
    }

    /// How many folders down `id` is (1 at the top), counted by its parents: a name may hold a
    /// `/` ([`Mailbox::names_may_hold_slashes`]).
    fn depth(&self, id: Id) -> usize {
        let mut depth = 0;
        let mut cur = Some(id);
        while let Some(f) = cur.and_then(|c| self.folder(c)) {
            depth += 1;
            cur = f.parent;
            if depth > MAX_DEPTH {
                break;
            }
        }
        depth
    }

    /// Whether the names of `account`'s folders may hold `/` (and `\`): a Microsoft account's
    /// server allows them, and its folders are known by their server ids, never by a path. Any
    /// other account's folders are named by their path on an IMAP server, where `/` parts them.
    pub fn names_may_hold_slashes(&self, account: Id) -> bool {
        self.account(account).is_some_and(|a| a.kind == AccountKind::Microsoft)
    }

    /// The folder of `account` that its server calls `id` ([`Folder::server`]).
    pub fn folder_by_server(&self, account: Id, id: &str) -> Option<Id> {
        if id.is_empty() {
            return None;
        }
        self.folders.iter().find(|f| f.account == account && f.server.as_ref().is_some_and(|s| s.id == id)).map(|f| f.id)
    }

    /// Remove exactly the folders `gone` with the mail in them: what a Microsoft account's server
    /// no longer has (R3 spec S8). A folder under one of them that isn't itself among them goes up
    /// to the top of its account rather than with it. Nothing is told to any server.
    pub fn remove_folders(&mut self, gone: &[Id]) {
        if gone.is_empty() {
            return;
        }
        let gone: HashSet<Id> = gone.iter().copied().collect();
        self.folders.retain(|f| !gone.contains(&f.id));
        for f in &mut self.folders {
            if f.parent.is_some_and(|p| gone.contains(&p)) {
                f.parent = None;
            }
        }
        self.messages.retain(|_, m| !gone.contains(&m.folder));
        for list in &mut self.favorites_from_server {
            list.folders.retain(|folder| !gone.contains(folder));
        }
        self.pending.retain(|p| !matches!(p, Pending::Move { to, .. } if gone.contains(to)));
    }

    fn name_taken(&self, account: Id, parent: Option<Id>, name: &str, except: Option<Id>) -> bool {
        self.folders
            .iter()
            .any(|f| f.account == account && f.parent == parent && Some(f.id) != except && f.name.to_lowercase() == name.to_lowercase())
    }

    /// Create a folder under `parent` (or at the account's top level).
    pub fn add_folder(&mut self, account: Id, parent: Option<Id>, name: &str) -> Result<Id, ModelError> {
        let name = clean_name(name).ok_or_else(|| invalid("a folder needs a name"))?;
        if name.contains('/') && !self.names_may_hold_slashes(account) {
            return Err(invalid("a folder name can't contain /"));
        }
        if self.account(account).is_none() {
            return Err(not_found("account", account));
        }
        if let Some(p) = parent {
            let pf = self.folder(p).ok_or(not_found("folder", p))?;
            if pf.account != account {
                return Err(invalid("the parent folder belongs to another account"));
            }
            if self.depth(p) >= MAX_DEPTH {
                return Err(invalid("folders are nested too deeply"));
            }
        }
        if self.name_taken(account, parent, &name, None) {
            return Err(invalid(format!("a folder named \"{name}\" already exists here")));
        }
        let id = self.alloc_id();
        self.folders.push(Folder { id, account, parent, name, ..Default::default() });
        Ok(id)
    }

    /// Rename a folder the user made (standard folders keep their names).
    pub fn rename_folder(&mut self, id: Id, name: &str) -> Result<(), ModelError> {
        let name = clean_name(name).ok_or_else(|| invalid("a folder needs a name"))?;
        let f = self.folder(id).ok_or(not_found("folder", id))?;
        if name.contains('/') && !self.names_may_hold_slashes(f.account) {
            return Err(invalid("a folder name can't contain /"));
        }
        if f.kind != FolderKind::Custom {
            return Err(invalid(format!("{} can't be renamed", f.name)));
        }
        if self.name_taken(f.account, f.parent, &name, Some(id)) {
            return Err(invalid(format!("a folder named \"{name}\" already exists here")));
        }
        self.folder_mut(id)?.name = name;
        Ok(())
    }

    /// Move a folder the user made under `parent` (or to the top level).
    pub fn move_folder(&mut self, id: Id, parent: Option<Id>) -> Result<(), ModelError> {
        let f = self.folder(id).ok_or(not_found("folder", id))?.clone();
        if f.kind != FolderKind::Custom {
            return Err(invalid(format!("{} can't be moved", f.name)));
        }
        if let Some(p) = parent {
            let pf = self.folder(p).ok_or(not_found("folder", p))?;
            if pf.account != f.account {
                return Err(invalid("a folder can't move to another account"));
            }
            if self.subtree(id).contains(&p) {
                return Err(invalid("a folder can't move into itself"));
            }
            if self.depth(p) + self.subtree(id).len() > MAX_DEPTH {
                return Err(invalid("folders are nested too deeply"));
            }
        }
        if self.name_taken(f.account, parent, &f.name, Some(id)) {
            return Err(invalid(format!("a folder named \"{}\" already exists there", f.name)));
        }
        self.folder_mut(id)?.parent = parent;
        Ok(())
    }

    /// Delete a folder the user made: it moves into Deleted Items with its messages; a folder
    /// already there is removed for good. Returns `true` when it was removed permanently.
    ///
    /// For an IMAP account none of this is pushed to the mail server (a mailbox there stays, and
    /// comes back as a folder with the next synchronisation); the changes still waiting to be
    /// pushed that mention a removed folder are dropped with it. For a Microsoft account the move
    /// into Deleted Items is the change its next run sends (the folder's parent differs from what
    /// the server last said), and a folder removed for good is queued for the server to delete
    /// ([`Queued::DeleteFolder`]), as Outlook does (ADR 0003).
    pub fn delete_folder(&mut self, id: Id) -> Result<bool, ModelError> {
        let f = self.folder(id).ok_or(not_found("folder", id))?.clone();
        if f.kind != FolderKind::Custom {
            return Err(invalid(format!("{} can't be deleted", f.name)));
        }
        let deleted = self.folder_of_kind(f.account, FolderKind::Deleted);
        let in_deleted = deleted.is_some_and(|d| self.subtree(d).contains(&id));
        match deleted {
            Some(d) if !in_deleted => {
                // A name already used in Deleted Items gets a number.
                let mut name = f.name.clone();
                let mut n = 1;
                while self.name_taken(f.account, Some(d), &name, Some(id)) && n < 1000 {
                    n += 1;
                    name = format!("{} ({n})", f.name);
                }
                let folder = self.folder_mut(id)?;
                folder.parent = Some(d);
                folder.name = name;
                folder.favorite = false;
                Ok(false)
            }
            _ => {
                let gone = self.subtree(id);
                let remotes: Vec<String> =
                    self.folders.iter().filter(|f| gone.contains(&f.id) && !f.remote.is_empty()).map(|f| f.remote.clone()).collect();
                // The server deletes what it has of them: each folder it has whose parent it
                // doesn't have among them (deleting that takes what is under it along).
                if self.names_may_hold_slashes(f.account) {
                    let on_server = |p: Id| gone.contains(&p) && self.folder(p).is_some_and(|parent| parent.server.is_some());
                    let deletes: Vec<Queued> = self
                        .folders
                        .iter()
                        .filter(|g| gone.contains(&g.id) && (g.id == id || !g.parent.is_some_and(on_server)))
                        .filter_map(|g| g.server.as_ref().map(|server| Queued::DeleteFolder { account: f.account, id: server.id.clone() }))
                        .collect();
                    self.queued.extend(deletes);
                }
                self.folders.retain(|f| !gone.contains(&f.id));
                self.messages.retain(|_, m| !gone.contains(&m.folder));
                for list in &mut self.favorites_from_server {
                    list.folders.retain(|folder| !gone.contains(folder));
                }
                self.pending.retain(|p| {
                    let into = matches!(p, Pending::Move { to, .. } if gone.contains(to));
                    let out_of = p.account() == f.account && remotes.iter().any(|remote| remote == p.source().0);
                    !into && !out_of
                });
                Ok(true)
            }
        }
    }

    // ---- messages ----

    /// Add a message to its folder (`folder` must exist); gives it an id when it has none.
    pub fn add_message(&mut self, mut m: Message) -> Result<Id, ModelError> {
        let folder = self.folder(m.folder).ok_or(not_found("folder", m.folder))?;
        m.account = folder.account;
        if m.id == 0 || self.messages.contains_key(&m.id) {
            m.id = self.alloc_id();
        }
        self.next_id = self.next_id.max(m.id.saturating_add(1));
        let id = m.id;
        self.messages.insert(id, Arc::new(m));
        Ok(id)
    }

    pub fn message(&self, id: Id) -> Option<&Message> {
        self.messages.get(&id).map(Arc::as_ref)
    }

    pub fn message_mut(&mut self, id: Id) -> Result<&mut Message, ModelError> {
        self.messages.get_mut(&id).map(Arc::make_mut).ok_or(not_found("message", id))
    }

    pub fn messages_in(&self, folder: Id) -> impl Iterator<Item = &Arc<Message>> {
        self.messages.values().filter(move |m| m.folder == folder)
    }

    pub fn count(&self, folder: Id) -> usize {
        self.messages_in(folder).count()
    }

    pub fn unread_count(&self, folder: Id) -> usize {
        self.messages_in(folder).filter(|m| !m.read).count()
    }

    /// The number a folder shows beside its name: unread messages, or everything for Drafts and
    /// Outbox (whatever waits there needs attention).
    pub fn badge(&self, folder: Id) -> usize {
        match self.folder(folder).map(|f| f.kind) {
            Some(FolderKind::Drafts | FolderKind::Outbox) => self.count(folder),
            Some(FolderKind::Deleted | FolderKind::Sent) | None => 0,
            Some(_) => self.unread_count(folder),
        }
    }

    /// Where `id` has a move waiting to be pushed to the server.
    fn waiting_move(&self, id: Id) -> Option<usize> {
        self.pending.iter().position(|p| matches!(p, Pending::Move { message, .. } if *message == id))
    }

    /// Move a message to another folder.
    ///
    /// A message the server has (it has a UID in a folder that follows a server mailbox) gets a
    /// [`Pending::Move`] for the next synchronisation to push; until then it has no UID in its
    /// new folder. One that already has a move waiting keeps it with the new destination, and
    /// moving it back to where the server still has it calls the move off. A message moved to
    /// another account's folder stays where it is on its own server: nothing is pushed.
    pub fn move_message(&mut self, id: Id, folder: Id) -> Result<(), ModelError> {
        let (account, remote) = {
            let f = self.folder(folder).ok_or(not_found("folder", folder))?;
            (f.account, f.remote.clone())
        };
        let m = self.message(id).ok_or(not_found("message", id))?;
        if m.folder == folder {
            return Ok(());
        }
        let (was_in, uid, synced) = (m.folder, m.uid, m.synced);
        let waiting = self.waiting_move(id);
        // What the message is to the server in its new folder: nothing, unless it is back.
        let (mut uid_there, mut synced_there, mut called_off) = (0, None, None);
        match waiting.and_then(|at| self.pending.get_mut(at)) {
            Some(Pending::Move { account: owner, from, uid, synced, to, .. }) => {
                if *owner == account && !remote.is_empty() && *from == remote {
                    // Back where the server has it all along.
                    (uid_there, synced_there, called_off) = (*uid, *synced, waiting);
                } else if *owner == account {
                    *to = folder;
                } else {
                    called_off = waiting;
                }
            }
            _ => {
                let from = self.folder(was_in).filter(|f| f.account == account && !f.remote.is_empty()).map(|f| f.remote.clone());
                if let (Some(from), true) = (from, uid != 0) {
                    self.pending.push(Pending::Move { account, message: id, from, uid, synced, to: folder });
                }
            }
        }
        if let Some(at) = called_off.filter(|at| *at < self.pending.len()) {
            self.pending.remove(at);
        }
        let m = self.message_mut(id)?;
        m.folder = folder;
        m.account = account;
        // The UID belonged to the old folder.
        m.uid = uid_there;
        m.synced = synced_there;
        Ok(())
    }

    /// Copy a message into `folder`; returns the copy's id. The copy is not on a server.
    pub fn copy_message(&mut self, id: Id, folder: Id) -> Result<Id, ModelError> {
        let mut copy = self.message(id).ok_or(not_found("message", id))?.clone();
        copy.id = 0;
        copy.folder = folder;
        copy.uid = 0;
        copy.synced = None;
        self.add_message(copy)
    }

    /// Remove a message for good, as its owner asks. When the server has it, a
    /// [`Pending::Delete`] tells the next synchronisation to delete it there too; a message with
    /// a move still waiting is deleted where the server has it.
    pub fn remove_message(&mut self, id: Id) -> Result<Message, ModelError> {
        let (was_in, account, uid, message_id) = {
            let m = self.message(id).ok_or(not_found("message", id))?;
            (m.folder, m.account, m.uid, m.message_id.clone())
        };
        match self.waiting_move(id).and_then(|at| self.pending.get_mut(at)) {
            Some(op) => {
                if let Pending::Move { account, from, uid, .. } = &mut *op {
                    let delete = Pending::Delete { account: *account, from: std::mem::take(from), uid: *uid, message_id };
                    *op = delete;
                }
            }
            None => {
                let from = self.folder(was_in).filter(|f| f.account == account && !f.remote.is_empty()).map(|f| f.remote.clone());
                if let (Some(from), true) = (from, uid != 0) {
                    self.pending.push(Pending::Delete { account, from, uid, message_id });
                }
            }
        }
        self.forget_message(id)
    }

    /// Remove a message without a word to the server: for a synchronisation taking out what the
    /// server no longer has. A move still waiting for the message is forgotten with it.
    pub fn forget_message(&mut self, id: Id) -> Result<Message, ModelError> {
        let m = self.messages.remove(&id).ok_or(not_found("message", id))?;
        for t in self.tasks.values_mut().filter(|t| t.message == Some(id)) {
            Arc::make_mut(t).message = None;
        }
        if !self.pending.is_empty() {
            self.pending.retain(|p| !matches!(p, Pending::Move { message, .. } if *message == id));
        }
        Ok(Arc::unwrap_or_clone(m))
    }

    /// Delete a message: it moves to Deleted Items, or is removed for good when it already is
    /// there (or the account has no such folder). Returns `true` when removed for good.
    pub fn delete_message(&mut self, id: Id) -> Result<bool, ModelError> {
        let m = self.message(id).ok_or(not_found("message", id))?;
        let deleted = self.folder_of_kind(m.account, FolderKind::Deleted);
        match deleted {
            Some(d) if !self.subtree(d).contains(&m.folder) => {
                self.move_message(id, d)?;
                Ok(false)
            }
            _ => {
                self.remove_message(id)?;
                Ok(true)
            }
        }
    }

    /// The messages of the conversation `id` is part of (same account, same topic), oldest first.
    pub fn conversation(&self, id: Id) -> Vec<Id> {
        let Some(m) = self.message(id) else { return Vec::new() };
        let key = m.thread_key();
        let deleted = self.folder_of_kind(m.account, FolderKind::Deleted);
        let junk = self.folder_of_kind(m.account, FolderKind::Junk);
        let mut v: Vec<&Arc<Message>> = self
            .messages
            .values()
            .filter(|x| x.account == m.account && x.thread_key() == key)
            // Deleted and junk mail stays out, unless that is where the conversation is read.
            .filter(|x| x.folder == m.folder || (Some(x.folder) != deleted && Some(x.folder) != junk))
            .collect();
        v.sort_by_key(|x| (x.date, x.id));
        v.into_iter().map(|x| x.id).collect()
    }

    /// Run the enabled rules on a message (new mail). Returns the names of the rules that ran.
    pub fn apply_rules(&mut self, id: Id) -> Vec<String> {
        let mut ran = Vec::new();
        for rule in self.rules.clone() {
            let Some(m) = self.message(id) else { break };
            if !rule.matches(m) {
                continue;
            }
            for action in &rule.actions {
                match action {
                    Action::MoveTo(folder) => {
                        // A rule pointing at a folder that is gone does nothing.
                        let _ = self.move_message(id, *folder);
                    }
                    Action::Delete => {
                        let _ = self.delete_message(id);
                    }
                    change => {
                        let Ok(m) = self.message_mut(id) else { break };
                        match change {
                            Action::Categorize(c) if !m.has_category(c) => m.categories.push(c.clone()),
                            Action::Flag => m.flag = crate::FlagStatus::Flagged,
                            Action::MarkRead => m.read = true,
                            Action::SetImportance(i) => m.importance = *i,
                            _ => {}
                        }
                    }
                }
            }
            ran.push(rule.name.clone());
            if rule.stop {
                break;
            }
        }
        ran
    }

    // ---- contacts, events, tasks ----

    pub fn add_contact(&mut self, mut c: Contact) -> Id {
        if c.id == 0 || self.contacts.contains_key(&c.id) {
            c.id = self.alloc_id();
        }
        self.next_id = self.next_id.max(c.id.saturating_add(1));
        let id = c.id;
        self.contacts.insert(id, Arc::new(c));
        id
    }

    pub fn contact(&self, id: Id) -> Option<&Contact> {
        self.contacts.get(&id).map(Arc::as_ref)
    }

    pub fn contact_mut(&mut self, id: Id) -> Result<&mut Contact, ModelError> {
        self.contacts.get_mut(&id).map(Arc::make_mut).ok_or(not_found("contact", id))
    }

    pub fn remove_contact(&mut self, id: Id) -> Result<(), ModelError> {
        self.contacts.remove(&id).map(|_| ()).ok_or(not_found("contact", id))
    }

    /// The contact with this address, if any.
    pub fn contact_by_email(&self, email: &str) -> Option<&Contact> {
        self.contacts.values().map(Arc::as_ref).find(|c| c.has_email(email))
    }

    pub fn calendar(&self, id: Id) -> Option<&Calendar> {
        self.calendars.iter().find(|c| c.id == id)
    }

    pub fn default_calendar(&self) -> Option<Id> {
        self.calendars.first().map(|c| c.id)
    }

    /// Add an event; without a (known) calendar it goes to the default one.
    pub fn add_event(&mut self, mut e: Event) -> Result<Id, ModelError> {
        if self.calendar(e.calendar).is_none() {
            e.calendar = self.default_calendar().ok_or_else(|| invalid("the mailbox has no calendar"))?;
        }
        if e.id == 0 || self.events.contains_key(&e.id) {
            e.id = self.alloc_id();
        }
        self.next_id = self.next_id.max(e.id.saturating_add(1));
        let id = e.id;
        self.events.insert(id, Arc::new(e));
        Ok(id)
    }

    pub fn event(&self, id: Id) -> Option<&Event> {
        self.events.get(&id).map(Arc::as_ref)
    }

    pub fn event_mut(&mut self, id: Id) -> Result<&mut Event, ModelError> {
        self.events.get_mut(&id).map(Arc::make_mut).ok_or(not_found("event", id))
    }

    pub fn remove_event(&mut self, id: Id) -> Result<(), ModelError> {
        self.events.remove(&id).map(|_| ()).ok_or(not_found("event", id))
    }

    pub fn add_task(&mut self, mut t: Task) -> Id {
        if t.id == 0 || self.tasks.contains_key(&t.id) {
            t.id = self.alloc_id();
        }
        self.next_id = self.next_id.max(t.id.saturating_add(1));
        let id = t.id;
        self.tasks.insert(id, Arc::new(t));
        id
    }

    pub fn task(&self, id: Id) -> Option<&Task> {
        self.tasks.get(&id).map(Arc::as_ref)
    }

    pub fn task_mut(&mut self, id: Id) -> Result<&mut Task, ModelError> {
        self.tasks.get_mut(&id).map(Arc::make_mut).ok_or(not_found("task", id))
    }

    pub fn remove_task(&mut self, id: Id) -> Result<(), ModelError> {
        self.tasks.remove(&id).map(|_| ()).ok_or(not_found("task", id))
    }

    // ---- categories ----

    pub fn category(&self, name: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.name.eq_ignore_ascii_case(name.trim()))
    }

    /// A category's colour; grey for a name the list doesn't have (a category from elsewhere).
    pub fn category_color(&self, name: &str) -> Rgb {
        self.category(name).map_or(Rgb(0x8A, 0x8A, 0x8A), |c| c.color)
    }

    /// Add a category, or recolour the one with that name.
    pub fn set_category(&mut self, name: &str, color: Rgb) -> Result<(), ModelError> {
        let name = clean_name(name).ok_or_else(|| invalid("a category needs a name"))?;
        match self.categories.iter_mut().find(|c| c.name.eq_ignore_ascii_case(&name)) {
            Some(c) => c.color = color,
            None => self.categories.push(Category { name, color }),
        }
        Ok(())
    }

    /// Rename a category everywhere it is used.
    pub fn rename_category(&mut self, from: &str, to: &str) -> Result<(), ModelError> {
        let to = clean_name(to).ok_or_else(|| invalid("a category needs a name"))?;
        if !from.eq_ignore_ascii_case(&to) && self.category(&to).is_some() {
            return Err(invalid(format!("a category named \"{to}\" already exists")));
        }
        let cat = self
            .categories
            .iter_mut()
            .find(|c| c.name.eq_ignore_ascii_case(from.trim()))
            .ok_or_else(|| invalid(format!("no category named \"{from}\"")))?;
        cat.name = to.clone();
        self.each_category_list(|list| {
            for c in list.iter_mut().filter(|c| c.eq_ignore_ascii_case(from.trim())) {
                *c = to.clone();
            }
        });
        Ok(())
    }

    /// Remove a category from the list and from every item.
    pub fn remove_category(&mut self, name: &str) -> Result<(), ModelError> {
        let before = self.categories.len();
        self.categories.retain(|c| !c.name.eq_ignore_ascii_case(name.trim()));
        if self.categories.len() == before {
            return Err(invalid(format!("no category named \"{name}\"")));
        }
        self.each_category_list(|list| list.retain(|c| !c.eq_ignore_ascii_case(name.trim())));
        Ok(())
    }

    /// Apply `f` to the category list of every item that has one (items without stay shared).
    fn each_category_list(&mut self, f: impl Fn(&mut Vec<String>)) {
        for m in self.messages.values_mut().filter(|m| !m.categories.is_empty()) {
            f(&mut Arc::make_mut(m).categories);
        }
        for c in self.contacts.values_mut().filter(|c| !c.categories.is_empty()) {
            f(&mut Arc::make_mut(c).categories);
        }
        for e in self.events.values_mut().filter(|e| !e.categories.is_empty()) {
            f(&mut Arc::make_mut(e).categories);
        }
        for t in self.tasks.values_mut().filter(|t| !t.categories.is_empty()) {
            f(&mut Arc::make_mut(t).categories);
        }
    }

    pub fn signature(&self, name: &str) -> Option<&Signature> {
        self.signatures.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }

    // ---- loading ----

    /// How many accounts the file said had their Favorites in Outlook read another way than
    /// through Microsoft Graph ([`Account::retired_favorites_choice`]), which is forgotten here:
    /// the owner is told once, and the next save writes nothing of it.
    pub fn take_retired_favorites_choices(&mut self) -> usize {
        let mut told = 0usize;
        for account in &mut self.accounts {
            told = told.saturating_add(usize::from(std::mem::take(&mut account.retired_favorites_choice)));
        }
        told
    }

    /// Make a mailbox read from a file consistent, whatever the file held: ids are unique and
    /// below `next_id`, every account has its standard folders, folder parents exist and don't
    /// loop, every message is in a folder, every event in a calendar. Returns what was fixed.
    pub fn repair(&mut self) -> Vec<String> {
        let mut fixed = Vec::new();
        // Ids stay within what JSON numbers hold exactly; anything above gets a new id, so the
        // counter can't run into the end of u64.
        let sane = |id: &Id| *id <= MAX_ID;
        let top = self
            .messages
            .keys()
            .chain(self.contacts.keys())
            .chain(self.events.keys())
            .chain(self.tasks.keys())
            .copied()
            .chain(self.accounts.iter().map(|a| a.id))
            .chain(self.folders.iter().map(|f| f.id))
            .chain(self.calendars.iter().map(|c| c.id))
            .chain(self.rules.iter().map(|r| r.id))
            .filter(sane)
            .max()
            .unwrap_or(0);
        self.next_id = if sane(&self.next_id) { self.next_id.max(top + 1) } else { top + 1 };
        /// Give items whose key is out of range a new one.
        fn rehome<T: Clone>(map: &mut BTreeMap<Id, Arc<T>>, next: &mut Id) -> usize {
            let big: Vec<Id> = map.range(MAX_ID + 1..).map(|(k, _)| *k).collect();
            for k in &big {
                if let Some(v) = map.remove(k) {
                    map.insert(*next, v);
                    *next += 1;
                }
            }
            big.len()
        }
        /// Map keys are the ids.
        fn rekey<T: Clone>(map: &mut BTreeMap<Id, Arc<T>>, id: impl Fn(&mut T) -> &mut Id) -> usize {
            let mut n = 0;
            for (k, v) in map.iter_mut() {
                if *id(Arc::make_mut(v)) != *k {
                    *id(Arc::make_mut(v)) = *k;
                    n += 1;
                }
            }
            n
        }
        let next = &mut self.next_id;
        let rehomed =
            rehome(&mut self.messages, next) + rehome(&mut self.contacts, next) + rehome(&mut self.events, next) + rehome(&mut self.tasks, next);
        let rekeyed = rekey(&mut self.messages, |m| &mut m.id)
            + rekey(&mut self.contacts, |c| &mut c.id)
            + rekey(&mut self.events, |e| &mut e.id)
            + rekey(&mut self.tasks, |t| &mut t.id);
        if rehomed + rekeyed > 0 {
            fixed.push(format!("{} item id(s) corrected", rehomed.max(rekeyed)));
        }
        // Accounts and folders with duplicate, missing or out-of-range ids get new ones.
        let mut seen = std::collections::BTreeSet::new();
        for c in &mut self.calendars {
            if c.id == 0 || !sane(&c.id) {
                c.id = self.next_id;
                self.next_id += 1;
            }
        }
        for a in &mut self.accounts {
            if a.id == 0 || !sane(&a.id) || !seen.insert(a.id) {
                a.id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                seen.insert(a.id);
                fixed.push("an account id corrected".into());
            }
        }
        let accounts: Vec<Id> = self.accounts.iter().map(|a| a.id).collect();
        let before = self.folders.len();
        self.folders.retain(|f| accounts.contains(&f.account));
        if self.folders.len() != before {
            fixed.push(format!("{} folder(s) of unknown accounts removed", before - self.folders.len()));
        }
        for f in &mut self.folders {
            if f.id == 0 || !sane(&f.id) || !seen.insert(f.id) {
                f.id = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                seen.insert(f.id);
                fixed.push("a folder id corrected".into());
            }
            if clean_name(&f.name).is_none() {
                f.name = f.kind.default_name().into();
            }
        }
        // Parents must exist, be in the same account and not loop.
        let snapshot = self.folders.clone();
        for f in &mut self.folders {
            let mut cur = f.parent;
            let mut steps = 0;
            let ok = loop {
                let Some(p) = cur else { break true };
                let Some(pf) = snapshot.iter().find(|x| x.id == p) else { break false };
                steps += 1;
                if pf.account != f.account || p == f.id || steps > MAX_DEPTH {
                    break false;
                }
                cur = pf.parent;
            };
            if !ok {
                f.parent = None;
                fixed.push(format!("folder \"{}\" moved to the top level", f.name));
            }
            // A nested or duplicate standard folder is an ordinary one.
            if f.kind != FolderKind::Custom && f.parent.is_some() {
                f.kind = FolderKind::Custom;
            }
        }
        for account in accounts {
            for kind in FolderKind::STANDARD {
                if !self.folders.iter().any(|f| f.account == account && f.kind == kind) {
                    let id = self.alloc_id();
                    self.folders.push(Folder { id, account, name: kind.default_name().into(), kind, ..Default::default() });
                    fixed.push(format!("{} recreated", kind.default_name()));
                }
                // Only one folder is the Inbox; a second one is an ordinary folder.
                for extra in self.folders.iter_mut().filter(|f| f.account == account && f.kind == kind).skip(1) {
                    extra.kind = FolderKind::Custom;
                }
            }
        }
        // Messages without a folder go to their account's Inbox, or the first one.
        let folder_account: BTreeMap<Id, Id> = self.folders.iter().map(|f| (f.id, f.account)).collect();
        let fallback = self.inbox();
        let mut homeless = Vec::new();
        for (id, m) in &mut self.messages {
            match folder_account.get(&m.folder) {
                Some(a) if *a == m.account => {}
                Some(a) => Arc::make_mut(m).account = *a,
                None => homeless.push(*id),
            }
        }
        if !homeless.is_empty() {
            fixed.push(format!("{} message(s) without a folder moved to the Inbox", homeless.len()));
        }
        for id in homeless {
            match fallback {
                Some(f) => {
                    let _ = self.move_message(id, f);
                }
                None => {
                    self.messages.remove(&id);
                }
            }
        }
        // What is kept about the server's copies must hang together: a UID names one message of
        // its folder, and the flags the server is said to have are those of that UID.
        let mut on_server: HashSet<(Id, u32)> = HashSet::new();
        let mut untied = 0;
        for m in self.messages.values_mut() {
            let twice = m.uid != 0 && !on_server.insert((m.folder, m.uid));
            if twice || m.synced.is_some_and(|s| s.uid != m.uid) {
                let m = Arc::make_mut(m);
                if twice {
                    m.uid = 0;
                }
                m.synced = None;
                untied += 1;
            }
        }
        if untied > 0 {
            fixed.push(format!("{untied} message(s) untied from the mail server's copy"));
        }
        let dropped = self.repair_pending(&on_server);
        if dropped > 0 {
            fixed.push(format!("{dropped} change(s) waiting for the mail server dropped"));
        }
        if self.calendars.is_empty() && !self.events.is_empty() {
            let id = self.alloc_id();
            self.calendars.push(Calendar { id, name: "Calendar".into(), color: Rgb(0x2F, 0x6F, 0xD0), visible: true, ..Default::default() });
        }
        if let Some(default) = self.default_calendar() {
            let known: Vec<Id> = self.calendars.iter().map(|c| c.id).collect();
            for e in self.events.values_mut().filter(|e| !known.contains(&e.calendar)) {
                Arc::make_mut(e).calendar = default;
            }
        }
        // The Favorites a server listed: one list an account, of folders that account has.
        let stated = std::mem::take(&mut self.favorites_from_server);
        let before: usize = stated.iter().map(|list| list.folders.len().saturating_add(1)).sum();
        for list in stated.into_iter().take(self.accounts.len().saturating_mul(2)) {
            if self.favorites_from_server.iter().all(|kept| kept.account != list.account) {
                self.set_server_favorites(list.account, Some(list.folders));
            }
        }
        let after: usize = self.favorites_from_server.iter().map(|list| list.folders.len().saturating_add(1)).sum();
        if after != before {
            fixed.push("the Favorites kept from a mail server corrected".into());
        }
        self.next_id = self.next_id.max(1);
        fixed
    }

    /// Keep of [`Mailbox::pending`] only what could have been noted here, and no more than
    /// [`MAX_PENDING`] of it: a file could otherwise have the server told to move or delete
    /// anything. `on_server` is every (folder, UID) a message has. Returns how many changes went.
    ///
    /// A change stays when it is about a mailbox one of its account's folders follows and a
    /// UID no message here still has, and no other change is about the same message; a move
    /// also needs its message to be in the folder it names, in that account, without a UID.
    fn repair_pending(&mut self, on_server: &HashSet<(Id, u32)>) -> usize {
        let before = self.pending.len();
        let mut kept: Vec<Pending> = Vec::new();
        let mut sources: HashSet<(Id, u32)> = HashSet::new();
        let mut moved: HashSet<Id> = HashSet::new();
        let mut mended = 0;
        for mut op in std::mem::take(&mut self.pending) {
            if kept.len() >= MAX_PENDING {
                break;
            }
            let account = op.account();
            let (from, uid) = op.source();
            let source = self.folders.iter().find(|f| f.account == account && !from.is_empty() && f.remote == from).map(|f| f.id);
            // UID 0: waiting for the copy it is about in a mailbox the server numbered anew, to
            // be found by its Message-ID.
            let waiting = uid == 0
                && match &op {
                    Pending::Move { message, .. } => self.messages.get(message).is_some_and(|m| !m.message_id.is_empty()),
                    Pending::Delete { message_id, .. } => !message_id.is_empty(),
                };
            let Some(source) = source.filter(|source| (uid != 0 || waiting) && !on_server.contains(&(*source, uid))) else { continue };
            if let Pending::Move { message, synced, to, .. } = &mut op {
                let fits = self.messages.get(message).is_some_and(|m| m.account == account && m.folder == *to && m.uid == 0);
                if !fits || *to == source || moved.contains(message) {
                    continue;
                }
                if synced.is_some_and(|s| s.uid != uid) {
                    *synced = None;
                    mended += 1;
                }
                moved.insert(*message);
            }
            if waiting || sources.insert((source, uid)) {
                kept.push(op);
            }
        }
        self.pending = kept;
        (before - self.pending.len()) + mended
    }
}
