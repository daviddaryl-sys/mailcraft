# MailCraft — Despres Connect edition

<img src="assets/branding/despres-connect/mailcraft-github.png" width="96" alt="MailCraft Despres Connect icon: an ivory folded envelope and coral seal on a deep teal tile">

**Mail, calendars, contacts and tasks in one open-source, native Rust app.**

MailCraft brings your email and daily planning together in a familiar desktop workspace. Organise messages, search your mail, manage calendars and keep contacts close, with a native egui interface and downloadable builds for Windows 11 PCs, Apple Silicon Macs and Intel Macs.

This independently maintained edition adds Despres Connect account setup, CalDAV/CardDAV synchronization, iCloud setup improvements and fixes that protect saved credentials when server settings change. It builds on the original MailCraft project, credited below.

Privacy is part of the design: mail stays in your local mailbox, message content runs no scripts, remote pictures load only when permitted, and remembered passwords use the operating system's credential store. Mail sync connects to your configured provider. Developers can also build the Rust code for other desktop platforms or WebAssembly, and automate actions through the CLI, local control channel or MCP server. The browser build imports files; it does not connect directly to mail servers.

> **Early alpha (0.4.3):** useful features are available, but provider compatibility and daily-use reliability are still being tested. Back up important mail and review the [known gaps](docs/gaps.md) and [roadmap](ROADMAP.md) before relying on it.

## Downloads for Windows and Mac

**Help shape MailCraft:** we are looking for our first five Mac testers. [Try it for ten minutes and leave feedback](TESTING.md), with sample mail and no real mailbox required.

**[Latest packaged build: MailCraft 0.4.3 — progressive sync fixes and our icon](https://github.com/daviddaryl-sys/mailcraft/releases/tag/v0.4.3-despres.2)**

| Your Mac | Download |
|---|---|
| Apple Silicon — M1, M2, M3, M4 or newer | [Download for Apple Silicon (universal DMG)](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.4.3-despres.2/mailcraft-0.4.3-macos-universal.dmg) |
| Intel | [Download for Intel (universal DMG)](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.4.3-despres.2/mailcraft-0.4.3-macos-universal.dmg) |

**Windows 11 (Intel/AMD x64):** [Download the installer (MSI)](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.4.3-despres.2/mailcraft-0.4.3-windows-x64.msi) · [Portable ZIP](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.4.3-despres.2/mailcraft-0.4.3-windows-x64-portable.zip).

Run the MSI to install, or extract the portable ZIP and open `mailcraft.exe`. Both include this edition's fixes and icon. The Windows build is unsigned, so Windows SmartScreen may show an unknown-publisher warning. These are x64 downloads; a native Windows ARM build is not included. Back up any existing app and mailbox data before upgrading. Windows build and smoke-test details are on the release page.

### Windows 11: unknown publisher warning

These early-alpha Windows builds are unsigned. If you downloaded MailCraft from [this repository's release page](https://github.com/daviddaryl-sys/mailcraft/releases/tag/v0.4.3-despres.2) and trust the source:

1. Open `mailcraft-0.4.3-windows-x64.msi`.
2. If **Windows protected your PC** appears, select **More info → Run anyway**.
3. If Windows asks whether to allow changes and shows **Unknown publisher**, check that it names the MailCraft installer, then select **Yes**.
4. Complete installation and open **MailCraft** from Start.

For the portable ZIP, extract it first and open `mailcraft.exe`; the same SmartScreen warning may appear.

If **Run anyway** is missing, or the warning mentions malware or **Smart App Control**, stop and check the exact message rather than switching off protection. Managed PCs may require an administrator's approval. Keep Microsoft Defender, SmartScreen and Smart App Control enabled. See [Microsoft's explanation of SmartScreen warnings](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation) and [Smart App Control](https://support.microsoft.com/en-us/windows/security/threat-malware-protection/smart-app-control-frequently-asked-questions).

### Mac installation

Check **Apple menu → About This Mac** if you are unsure: “Chip” identifies Apple Silicon; “Processor” naming Intel means the Intel download. The same universal app runs on both types of Mac and requires macOS 11 or later. Open the DMG, quit and back up both your existing MailCraft app and its mailbox data, then drag `MailCraft.app` into Applications. No Rust or Homebrew installation is required.

These are early-alpha builds with local ad-hoc signatures, **not Apple-notarized**. macOS may require approval in **System Settings → Privacy & Security**; managed Macs may block them. Downloads contain the app and its licences, with no saved accounts, passwords or private mailbox data. Configure your own accounts after installation. [Checksums](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.4.3-despres.2/SHA256SUMS.txt) and verification details are on the release page. Intel's version check passed under Rosetta; physical Intel hardware and older macOS versions have not been tested.

## What this version improves

This edition builds on the original MailCraft foundation with work focused on account setup, calendar/contact sync and credential handling:

- **Despres Connect setup:** email-first setup configures mail, calendar and contacts together. Client domains are checked against the Despres Connect domain-discovery service before ordinary provider discovery; no client-domain list is built into the app. Manual setup remains available. New accounts use `https://outlook.despresai.com/dav/`; existing accounts retain their saved settings. [Setup details](docs/automatic-setup.md).
- **CalDAV and CardDAV:** shared or separate calendar/contact endpoints, discovery of collections and write permissions, and conditional synchronization with conflict checks. An unreadable new server item is left on the server and skipped so it cannot stall the whole download. Personal Contacts is preferred over recipient-cache collections. [Scope and limits](docs/dav-sync.md).
- **Progressive first sync:** verified calendar events and contacts appear and save in batches while the first import runs. Progress stays visible, local edits and deletions are retained, and failed automatic passes back off from five to thirty minutes. Manual sync remains available.
- **iCloud setup:** separate Apple calendar/contact service roots, constrained Apple server discovery, app-specific-password guidance, advertised IMAP SASL PLAIN support, and a fix for blocked account-dialog input. [iCloud details and verification limits](docs/icloud-setup.md).
- **Credential protection:** changing an account's mail, calendar or separate contacts server clears its saved password/sign-in, including the OS credential-store entry, so a server change cannot silently carry remembered credentials to another endpoint.

The 0.4.3 release integrates upstream improvements through `77a3253`: Microsoft Graph mail, agent permissions, privacy-safe logging, storage and search improvements, and better character decoding, while retaining this edition's fixes and branding. All seven repository CI checks passed: **2,194 tests passed, 10 ignored**, plus formatting, clippy, asset attribution, layering, WASM and dependency-policy checks. Migration was verified offline against backed-up mailbox copies, and the Apple Silicon app was checked with three configured accounts. This is not a claim of compatibility with every provider or Mac.

**Upgrading from 0.3?** Back up the app and its mailbox data before launching 0.4.3. It upgrades the mailbox to storage format 3; an older app needs the matching older data backup to roll back. Review the [known gaps](docs/gaps.md) before relying on this early-alpha software for important mail.

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

This edition has packaged Windows x64 and Mac downloads above. Other platforms can be built from source; upstream binaries do not necessarily include this edition's changes. See [packaging documentation](docs/releasing.md) for the shared build tooling.

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

**Everything CI runs:** `cargo xtask ci` (formatting, clippy with warnings as errors, the tests, asset attribution, crate layering, the wasm build, and dependency advisories, licences and sources).

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
