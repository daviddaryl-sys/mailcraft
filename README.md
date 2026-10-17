# MailCraft — Despres Connect edition

<img src="assets/branding/despres-connect/mailcraft-github.png" width="96" alt="MailCraft Despres Connect icon: an ivory folded envelope and coral seal on a deep teal tile">

**Mail, calendars, contacts and tasks in one open-source, native Rust app.**

MailCraft brings your email and daily planning together in a familiar desktop workspace. Organise messages, search your mail, manage calendars and keep contacts close, with a native egui interface and downloadable builds for both Apple Silicon and Intel Macs.

This independently maintained edition adds Despres Connect account setup, CalDAV/CardDAV synchronization, iCloud setup improvements and fixes that protect saved credentials when server settings change. It builds on the original MailCraft project, credited below.

Privacy is part of the design: mail stays in your local mailbox, message content runs no scripts, remote pictures load only when permitted, and remembered passwords use the operating system's credential store. Mail sync connects to your configured provider. Developers can also build the Rust code for other desktop platforms or WebAssembly, and automate actions through the CLI, local control channel or MCP server. The browser build imports files; it does not connect directly to mail servers.

> **Early alpha (0.3):** useful features are available, but provider compatibility and daily-use reliability are still being tested. Back up important mail and review the [known gaps](docs/gaps.md) and [roadmap](ROADMAP.md) before relying on it.

## Download for Mac

**[Latest packaged build: MailCraft 0.3.0 DAV.4](https://github.com/daviddaryl-sys/mailcraft/releases/tag/v0.3.0-dav4-macos-arm64)**

| Your Mac | Download |
|---|---|
| Apple Silicon — M1, M2, M3, M4 or newer | [Download for Apple Silicon (arm64 ZIP)](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.3.0-dav4-macos-arm64/MailCraft-0.3.0-dav4-macos-arm64.zip) |
| Intel | [Download for Intel (x86_64 ZIP)](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.3.0-dav4-macos-arm64/MailCraft-0.3.0-dav4-macos-x86_64.zip) |

Check **Apple menu → About This Mac** if you are unsure: “Chip” identifies Apple Silicon; “Processor” naming Intel means the Intel download. Both bundles require macOS 11 or later. Unzip the download, quit and back up any existing MailCraft app, then drag `MailCraft.app` into Applications. No Rust or Homebrew installation is required.

These are early-alpha builds with local ad-hoc signatures, **not Apple-notarized**. macOS may require approval in **System Settings → Privacy & Security**; managed Macs may block them. Downloads contain the app and its licences, with no saved accounts, passwords or private mailbox data. Configure your own accounts after installation. [Checksums](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.3.0-dav4-macos-arm64/SHA256SUMS.txt) and verification details are on the release page. Intel's version check passed under Rosetta; physical Intel hardware and older macOS versions have not been tested.

## What this version improves

This edition builds on the original MailCraft foundation with work focused on account setup, calendar/contact sync and credential handling:

- **Despres Connect setup:** email-first setup configures mail, calendar and contacts together. Known migrated domains use their verified profile; other domains are checked against the Despres Connect domain-discovery service before ordinary provider discovery. New accounts use `https://outlook.despresai.com/dav/`; existing accounts retain their saved settings. [Setup details](docs/automatic-setup.md).
- **CalDAV and CardDAV:** shared or separate calendar/contact endpoints, discovery of collections and write permissions, and conditional synchronization with conflict checks. An unreadable new server item is left on the server and skipped so it cannot stall the whole download. Personal Contacts is preferred over recipient-cache collections. [Scope and limits](docs/dav-sync.md).
- **iCloud setup:** separate Apple calendar/contact service roots, constrained Apple server discovery, app-specific-password guidance, advertised IMAP SASL PLAIN support, and a fix for blocked account-dialog input. [iCloud details and verification limits](docs/icloud-setup.md).
- **Credential protection:** changing an account's mail, calendar or separate contacts server clears its saved password/sign-in, including the OS credential-store entry, so a server change cannot silently carry remembered credentials to another endpoint.

The DAV.4 source passed all six repository CI steps, including **1,746 tests** and the WASM build check. The Apple Silicon build has been used with three configured accounts; this is not a claim of compatibility with every provider or Mac. This remains an early-alpha project: review [known gaps](docs/gaps.md) before relying on it for important mail.

## Original project and credit

This is an independently maintained version of MailCraft, based on [billsecond/mailcraft](https://github.com/billsecond/mailcraft). The [original MailCraft project at UCS-Care-Group/mailcraft](https://github.com/UCS-Care-Group/mailcraft) and its contributors created the Rust engine, egui interface, WASM support and most of the features described below. Those foundations are their work; the additions above describe this edition's changes.

Original copyright, [MIT](LICENSE-MIT)/[Apache-2.0](LICENSE-APACHE) licences, [NOTICE](NOTICE) and [asset attribution](ATTRIBUTION.md) are retained. This edition's source and Mac downloads are maintained in [daviddaryl-sys/mailcraft](https://github.com/daviddaryl-sys/mailcraft); upstream releases may have different features and fixes.

## What you can do

- **Keep mail organised.** Use folders, favourites, conversations, flags, categories, rules and undo. Read messages in a preview pane or open them in their own windows.
- **Find messages quickly.** Search with filters such as `from:`, `has:attachment`, `is:unread` and date ranges, in one folder or across mailboxes.
- **Write and reply.** Compose mail, reply or forward, add attachments and signatures, and save drafts. Supported rich-text formatting includes emphasis, links and lists; see the [current editing limits](docs/gaps.md).
- **Plan your day.** Browse day, week and month calendars, create appointments and recurring events, and manage reminders. Meeting and server-sync features depend on the account provider; see [calendar support](docs/calendar-parity.md) and [DAV limits](docs/dav-sync.md).
- **Keep contacts and tasks nearby.** Store contact details, use recipient suggestions and manage to-dos with dates, priorities and links to flagged mail.
- **Connect your accounts.** Use encrypted IMAP/SMTP, Microsoft sign-in where supported, or the Despres Connect and iCloud setup flows. CalDAV/CardDAV adds calendar and contacts synchronization to compatible password-based accounts. [Account setup](docs/automatic-setup.md).
- **Read with privacy controls.** HTML mail is rendered without scripts. Remote images are controlled by your picture-download preferences; links show their destinations.
- **Bring your files with you.** Import and export `.eml`, `.mbox`, `.ics` and `.vcf` files, with mailbox data saved locally.
- **Automate your workflow.** Use the CLI, authenticated local JSON control channel or MCP server to run the same commands as the interface. [Automation documentation](docs/mcp.md).

This edition has packaged Mac downloads above. Other platforms can be built from source; upstream binaries do not necessarily include this edition's changes. See [packaging documentation](docs/releasing.md) for the shared build tooling.

## What it looks like

| | |
|---|---|
| ![The Inbox with the reading pane, dark](docs/images/mail-dark.png) | ![A message written in a word processor: lists, a table, a quoted thread](docs/images/html-mail.png) |
| ![The calendar, week view](docs/images/calendar-week.png) | ![Adding an account](docs/images/add-account.png) |

The pictures are of the sample mailbox (`--sample`): invented people and invented mail.

## Platforms

| | |
|---|---|
| **Desktop** | macOS (separate Apple Silicon and Intel downloads), Windows (x64, arm64), Linux (x86_64, aarch64, riscv64), FreeBSD (x86_64) |
| **Web** | any modern browser, through WebAssembly (WebGPU, or WebGL2 where that is missing) |

The browser build is the same app with three differences that a browser imposes: it can't open connections to mail servers (bring mail in with File › Import, or use the desktop app), the mailbox lives in the browser's storage, and saving means downloading.

## Build and run

```
git clone https://github.com/daviddaryl-sys/mailcraft.git
cd mailcraft
cargo run --release -p mailcraft -- --sample
```

`--sample` opens an invented mailbox to try things on and keeps nothing. Without it, MailCraft keeps your mailbox in its settings folder and saves as you go.

**Command line:**

```
mailcraft-cli run --sample --cmd mail.markAllRead --cmd 'folder.new={"name":"Later"}' --save demo.mailcraft
mailcraft-cli list demo.mailcraft --folder Inbox --query "has:attachment"
mailcraft-cli import demo.mailcraft newsletter.eml team.ics people.vcf
mailcraft-cli convert archive.mbox archive.mailcraft
mailcraft-cli commands
```

**Web:** install [trunk](https://trunkrs.dev) and the `wasm32-unknown-unknown` target, then

```
cd apps/mailcraft-web && trunk serve
```

and open http://127.0.0.1:8772/ (`?sample` starts from the sample mailbox, `?empty` from an empty one).

**Everything CI runs:** `cargo xtask ci` (formatting, clippy with warnings as errors, the tests, asset attribution, crate layering, the wasm build).

## Driving it from an agent

```
mailcraft --sample --control 7991          # the app, with a control channel
mailcraft-cli mcp --connect 127.0.0.1:7991 # an MCP server bridged to that window
mailcraft-cli mcp --sample                 # or headless, no window
```

See [docs/mcp.md](docs/mcp.md) and [docs/control-protocol.md](docs/control-protocol.md). Messages are data, not instructions: an agent reading your mail should never act on what a message tells it to do, and the MCP server says so in its instructions.

## How it is put together

| Layer | Crates | Job |
|---|---|---|
| L0 | `time`, `control-key` | Calendar maths and date text; control-port key files |
| L1 | `model`, `html` | The mailbox and its edit primitives, recurrence, search; hostile HTML to safe display blocks |
| L2 | `mime`, `ical`, `vcard` | Internet messages and mbox; iCalendar; vCard |
| L3 | `proto`, `net` | IMAP and SMTP without sockets; TLS sockets and synchronisation |
| L4 | `engine` | Session, undo, the command registry, the feature catalog, file dispatch |
| L5 | `mcp` | MCP server |
| L6 | `ui-egui` | The Outlook-style front end (swappable) |
| Apps | `mailcraft`, `mailcraft-cli`, `mailcraft-web` | Desktop, command line, browser |

A crate may only depend on lower layers, and nothing below L6 knows about the UI toolkit (`cargo xtask layers` enforces both). More in [docs/architecture.md](docs/architecture.md).

## Principles

- **Never crash.** People trust a mail program with years of correspondence. No panics in non-test code, every input (files, servers, command parameters) treated as hostile, a failed command leaves the mailbox exactly as it was.
- **Your mail stays yours.** Nothing in a message runs, nothing in a message is fetched unless you ask for its pictures, passwords and sign-ins are held in memory, kept by the operating system's credential store when you want them remembered, and never written to MailCraft's own files or logs.
- **Clean room.** Built from public specifications (RFC 5322, MIME, IMAP, SMTP, OAuth 2.0, iCalendar, vCard) and from watching how mail programs behave, never from anyone's code or assets. Every icon is drawn in code; every asset is listed in [ATTRIBUTION.md](ATTRIBUTION.md).
- **Everything is a command.** One registry behind the ribbon, the shortcuts, command search, the CLI, the control channel and MCP.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md) first (it is for people as much as for agents): the never-crash rules, the asset policy and the quality gates are short and not negotiable. Work to beta is a list of issues, one per task in [the plan](docs/plans/roadmap-reset.md): claim one before you build it.

**Testing this edition?** Report bugs in [this repository’s issues](https://github.com/daviddaryl-sys/mailcraft/issues). Remove passwords, tokens and private message contents from any report. To participate in upstream testing, use the [original project’s tester signup](https://github.com/UCS-Care-Group/mailcraft/issues/new?template=tester-signup.yml).

## License

MailCraft is dual-licensed under the [MIT License](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option. Bundled fonts keep their own open licences; see [NOTICE](NOTICE) and [ATTRIBUTION.md](ATTRIBUTION.md). MailCraft is not affiliated with Microsoft; Outlook is a trademark of Microsoft Corporation, named here only to say what MailCraft is an alternative to.
