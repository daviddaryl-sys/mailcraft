# MailCraft vs Outlook: the assessment

> **Last reviewed:** 2026-10-14 · **Last updated:** 2026-10-17 · **Change:** minor (local ICLOUD.1: iCloud mail, calendar and contacts setup; separate DAV origins) · **Target:** Microsoft Outlook (Microsoft 365, classic desktop)

Local iCloud build (ICLOUD.1, 2026-10-17): separate CalDAV/CardDAV endpoints, Apple app-specific
password guidance and constrained Apple partition discovery. Verification status and limits are
in [icloud-setup.md](icloud-setup.md).


Local Despres Connect build (AUTO.1, 2026-10-17): email-first setup carries the shared DAV URL
with mail settings, checks all three services with one credential, and starts their downloads.
See [automatic-setup.md](automatic-setup.md) for supported discovery and remaining provider limits.


> **Out of date.** Written on 2026-10-11 and not kept up since: pictures in mail, invitations,
> reminders that fire and much else have landed. For what is true now, see [`gaps.md`](gaps.md)
> and [`ROADMAP.md`](../ROADMAP.md). This page is folded into those two in step R8.3 of the
> [roadmap reset](plans/roadmap-reset.md).

What MailCraft can and can't do next to the program it is an alternative to. The measured number
is the ribbon/menu catalog ([`parity-checklist.md`](parity-checklist.md)); everything else here
is a judgement, said plainly. The ranked work list is [`gaps.md`](gaps.md).

## Methodology

- **Measured:** `crates/engine/src/catalog.rs` lists 310 ribbon and menu features of the target by
  name, each mapped to a command id. `cargo xtask parity` counts how many ids exist in the
  registry. A command existing means the behaviour is reachable; it says nothing about depth.
- **Estimated:** "ready for real work" weighs what a person does in a week (reading and filing
  mail, answering, scheduling, finding things) by how completely MailCraft does it, then discounts
  for what stops them getting started (connecting an account) and what would make them leave
  (losing server state, not seeing a message as sent). No user evidence yet: nobody outside the
  authors has used it.

## Headline (2026-10-11)

| Number | Value | Kind |
|---|---|---|
| Ribbon/menu catalog coverage | **162 / 310 (52.3%)** | measured |
| Commands | 143 | measured |
| Ready for real work, with a real account | **~15%** | estimated: the local experience is far ahead of the server side |
| Ready as a local viewer/organiser of exported mail (`.mbox`, `.eml`, `.ics`, `.vcf`) | **~55%** | estimated |

## By area

| Area | Where it stands |
|---|---|
| **Reading and triage** | Strong for an alpha: virtualised list, date groups, sort, filters, conversations, pin, flags with due dates, categories, importance, junk, archive, two-step delete, full undo, keyboard movement. Missing: drag and drop, column view, Focused/Other, ignore/clean up conversation, reminders on mail |
| **Search** | Query language with field filters, negation and dates, one folder or everywhere; instant on tens of thousands of messages. Missing: search folders, saved searches, indexing for very large mailboxes, searching attachments' contents |
| **Rendering mail** | Safe by construction and readable, but simplified: no layout, colours or pictures (gap #6) |
| **Writing mail** | Works end to end in plain text with attachments, drafts, signatures, name checking. No rich text, spell check, inline pictures, templates, delayed send, receipts (gap #7) |
| **Rules** | Six conditions, six actions, stop-processing, run now, create from a message. No exceptions, no "sent to me only", no server-side rules, no alerts |
| **Servers** | IMAP download and SMTP send with TLS/STARTTLS, UID bookkeeping, special-use folder mapping, hostile-server hardening. Two-way for read marks, flags, moves, deletes and sent copies; Microsoft sign-in (OAuth 2.0) with the provider recognised from the address; untested against real servers, absent in the browser (gaps #1–#5) |
| **Calendar** | Day/work-week/week/month, recurrence (the common rules, with counts, end dates and skipped days), all-day and multi-day events, reminders stored, show-as, private, several calendars, `.ics` in and out, time zones (the calendar's, an item's own, a second one in the time bar). No invitations by mail, no free/busy, no shared calendars, no reminders firing (gaps #9–#11) |
| **People** | Contacts with several addresses/phones, favourites, categories, `.vcf` in and out, add-sender, name checking. No groups, photos, linked contacts, directory lookup |
| **Tasks** | To-dos with dates, importance, categories, completion, links to flagged mail. No recurrence, assignment, status reports, To-Do bar |
| **Files** | `.eml`, `.mbox`, `.ics`, `.vcf`, own `.mailcraft`. No `.pst`/`.ost`/`.msg`/`.olm` (gap #12) |
| **Platforms** | Compiled and run here: Windows x64 and the browser build. macOS, Linux, BSD and ARM builds are wired through the shared Craft workflows and have not run yet (gap #18) |
| **Agents** | Ahead of the target: every action is a command with a documented id and parameters, reachable from a CLI, a keyed local control channel and MCP, with a headless mode |
| **Privacy and safety** | Ahead of the target by default: nothing in a message runs or is fetched, passwords and sign-ins are kept only by the operating system's credential store, unencrypted server connections are refused |

## What "beta" means here

A person can connect a mainstream hosted account, read and answer mail for a week on two devices
without anything going out of step, and accept a meeting. That is gaps #1–#4, #6 (at least inline
pictures and basic layout), #7 (at least rich replies), #9 and #11.

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-11 | major | First assessment |
| 2026-10-14 | minor | Marked out of date |

| 2026-10-17 | minor | AUTO.1 local account setup; explicit DAV/provider limits |

| 2026-10-17 | minor | ICLOUD.1: separate Apple DAV setup and limits |
