# iCloud setup in the local MailCraft build

> **Status:** local ICLOUD.3 · **Updated:** 2026-10-17 · **Change:** password entry stays above the modal overlay

Add Account recognizes `icloud.com`, `me.com` and `mac.com`, and supports iCloud selected
manually or recognized by its MX records. Mail uses `imap.mail.me.com:993` with TLS and
`smtp.mail.me.com:587` with mandatory STARTTLS. Calendar starts at `https://caldav.icloud.com/`;
contacts at `https://contacts.icloud.com/`. Apple's public DNS SRV responses advertised those
two HTTPS services on 17 October 2026; neither advertised a TXT path. DAV discovery obtains
the user's principal and home URLs rather than inventing an account number or partition.

Use an Apple **app-specific password**, generated in Apple Account security settings with
two-factor authentication enabled. MailCraft's UI links to Apple's instructions. It does not
accept an Apple web session, implement Apple's app authorization program or ask for an OTP.
The password is used in memory for mail and both DAV checks, and kept only in the existing
OS credential store when that preference is enabled. Never put it in a terminal, control
request, mailbox file, chat, report or source fixture. A different DAV/Apple Account login can
be entered in Change account settings; aliases/custom-domain mail may also need the primary
iCloud mail login in User name.

All configured services must authenticate and expose a calendar and address book before the
account is added. Initial DAV download is read-only. Failure does not add a half-configured
account. Revoking an app-specific password or changing the Apple Account password requires
a new app-specific password. A user without iCloud Mail enabled cannot connect all three
services through this all-three setup flow.

## Password entry

The local ICLOUD.3 update keeps dialogs on a layer above the full-window blocking overlay.
Previously clicking outside the dialog could raise that overlay over its inputs, leaving
the password empty even when the owner clicked and typed. A pointer-driven regression
reproduces the empty field before the change and verifies password entry and Connect in
both themes. Synthetic keyboard-focus tests alone did not catch the layer-order problem.

## IMAP password authentication

On 17 October 2026 Apple's encrypted IMAP greeting advertised `AUTH=PLAIN` and `SASL-IR`.
The first local build used IMAP LOGIN and the owner's sign-in received
`AUTHENTICATIONFAILED`. The local update prefers advertised SASL PLAIN (RFC 4616), sending
an initial response only when SASL-IR is advertised (RFC 4959). Servers without advertised
PLAIN retain LOGIN. A refusal is final: there is no second mechanism or credential retry.
Credentials and their base64 representation are redacted; extra challenges are cancelled
once and bounded; success before any credential was sent is refused. Transport encryption
requirements are unchanged. This adds protocol compatibility; it does not establish that
Apple accepts the owner's credentials. Live iCloud mail/calendar/contact sync remains
unverified until the owner signs in through the native app.

## Connection boundaries

The existing shared Despres DAV connection retains its paths and strict same-origin policy.
Separate endpoints use absolute resource/collection identifiers so identical paths on two
servers cannot collide. Calendar traffic is never sent to the contacts origin or vice versa.

Only the explicitly recognized Apple calendar/contacts roots and their corresponding numeric
partition service hosts are trusted for authenticated read redirects and DAV hrefs. They must
use HTTPS on port 443 and pass normal certificate validation. No arbitrary `*.icloud.com`
wildcard is trusted. Redirects are limited to five, loops/missing/ambiguous locations fail,
and no conditional PUT/DELETE is redirected. A server migration that needs a write redirect
retains edits for resolution instead of forwarding an ETag to a new resource. Other providers
keep strict same-origin credentials. Mainland China `.com.cn` routing is not implemented.

Both inventories must finish before any writes. If either fails, local data/checkpoints remain.
Changes to either configured endpoint or DAV identity invalidate an in-flight result. Undo
rebasing and disconnect account for both connections. Existing personal-contact/appointment
write semantics, conflict protections, unknown-field preservation and meeting restrictions
remain as documented in [dav-sync.md](dav-sync.md).

## Verification

Tests use synthetic loopback servers only: separate origins with identical resource paths,
conditional writes to the correct server, incomplete inventory without partial writes,
trusted read redirects, rejection of foreign destinations/write redirects, redirect loops,
Apple host/port/TLS bounds, stale endpoint/identity results and one app-specific password
through account setup. Public unauthenticated Apple endpoint/DNS observations establish
service availability only; a real account is connected solely through the owner's native
password entry. Do not claim successful live iCloud synchronization until it is observed.
Meeting invitations, tasks and other existing DAV limits remain.

Primary sources: [Apple mail settings](https://support.apple.com/en-us/102525),
[Apple app-specific passwords](https://support.apple.com/en-us/102654),
[Apple third-party app access](https://support.apple.com/en-us/121539),
[RFC 6764](https://www.rfc-editor.org/rfc/rfc6764.html),
[RFC 4791](https://www.rfc-editor.org/rfc/rfc4791.html),
[RFC 6352](https://www.rfc-editor.org/rfc/rfc6352.html).


## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-17 | ICLOUD.1 | Separate Apple DAV services and constrained partition discovery |
| 2026-10-17 | ICLOUD.2 | Advertised IMAP SASL PLAIN, bounded exchanges and secret redaction |

| 2026-10-17 | ICLOUD.3 | Keep dialogs above the blocking overlay; pointer-entry regression in both themes |
