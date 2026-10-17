# CalDAV and CardDAV in the local MailCraft build

This original implementation adds a DAV connection to a password-based mail account. It uses the existing OS credential store, and never writes passwords into the mailbox file. It does not implement Exchange ActiveSync.

## Connection

Account Settings has an optional **Calendar and contacts (DAV URL)** field. For the configured Despres Connect service this is `https://outlook-test.computerhelpexpert.co.uk/dav/`. Discovery finds the authenticated principal, calendar/address-book homes, collections and their write privileges. No server paths are guessed from a collection's display name. The first sync downloads only.

`ui.davSync` accepts `{"account":10,"readOnly":true}` for read-only discovery and download. Without `readOnly`, subsequent runs send pending changes and receive server changes. The account's existing mail password is reused internally. No password belongs in a command, terminal history, report or source file.

## Two-way behavior

- Contacts and personal appointments can be created, edited and deleted. New appointments use the selected server calendar, and new contacts use the writable personal Contacts book (or the single writable address book). `people.new` can explicitly select `account` and `addressBook`. Existing local data and imported files are not automatically uploaded.
- The desktop checks periodically and shortly after local edits. Downloads are cached in the regular mailbox file and survive restart.
- Exact strong ETags protect PUT and DELETE. Creation uses a stable UID and `If-None-Match: *`. A conflicting server edit does not cause an unconditional retry or overwrite.
- Only a complete unfiltered collection inventory establishes a remote deletion. Invalid XML, missing properties, denied access and interrupted downloads retain local data/checkpoints.
- Original ICS/vCard documents are retained. Editing known fields preserves unknown properties and components, contact photos, custom labels, and unchanged time-zone data. XML processing refuses DTDs/entities outside XML's built-ins, and enforces structural and byte limits.
- Server-confirmed DAV changes settle their Undo steps. Unrelated mail Undo steps are preserved and rebased onto current DAV data, preventing an old mailbox snapshot from turning a download into deletions.
- `dav.status` reports collection permissions and pending changes. `dav.useServer` preserves edited local copies separately and arranges a fresh download of the server copy. `dav.disconnect` preserves downloaded data locally and changes nothing on the server.

## Deliberate limits

- Meeting creation, edits, deletion and replies involving ORGANIZER or ATTENDEE are blocked because a DAV server may automatically send invitations/cancellations. Use Despres Connect for meetings.
- Edit detached recurrence instances, move appointments between server calendars, and introduce a new event time zone in Despres Connect. Whole personal recurring series and their supported fields can be edited when stored as one VEVENT.
- Tasks, address-book/calendar creation and rename, attachments, delegation, free/busy and server-side meeting scheduling are not implemented by this DAV connection.
- A configured HTTPS DAV context URL is required. Redirects and cross-origin principal/home/resource URLs are refused; credentials never follow them. This has not established iCloud compatibility: iCloud may need separately configured calendar/contact hosts and explicit handling of its service redirects. Its IMAP preset already exists independently.
- Limits: 256 collections, 20,000 resources, 2 MiB per document, 16 MiB per XML reply, 64 MiB raw DAV data per account. Unsupported input is retained rather than silently rewritten.

## Verification

Write tests use a synthetic loopback DAV server only. They exercise discovery, permissions, download, conditional edits/creation/deletion, 412 conflicts, incomplete inventories, unknown property/photo preservation, restart recovery, hostile XML/hrefs, in-flight edits/deletes and Undo safety. Live verification must only download existing account data; it must not create test events/contacts or send mail.

Protocol references and grommunio scheduling observations are in [dav-protocol-research.md](dav-protocol-research.md).
