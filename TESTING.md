# Try MailCraft on your Mac

We are looking for our first **five volunteer Mac testers**, including both Apple Silicon and Intel users. Try the app for ten minutes and tell us where you get stuck. No purchase or Apple Developer membership is required to download it.

MailCraft is an open-source mail, calendar, contacts and tasks app written in Rust. This edition adds Despres Connect setup, CalDAV/CardDAV synchronization, iCloud setup improvements and credential-protection fixes on top of the original MailCraft project.

## 1. Download and install

- [Apple Silicon: M1 or newer](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.3.0-brand2-macos/MailCraft-0.3.0-brand2-macos-arm64.zip)
- [Intel Mac](https://github.com/daviddaryl-sys/mailcraft/releases/download/v0.3.0-brand2-macos/MailCraft-0.3.0-brand2-macos-x86_64.zip)
- [Release notes and checksums](https://github.com/daviddaryl-sys/mailcraft/releases/tag/v0.3.0-brand2-macos)

Both bundles declare macOS 11 or later; older macOS versions and physical Intel hardware have not been runtime-tested. Check Apple menu > About This Mac for your chip/processor. Unzip the download and drag MailCraft.app into Applications. Quit and back up any previous version first.

These early-alpha builds are not Apple-notarized. If macOS cannot verify the developer, first attempt to open MailCraft, dismiss the warning, then use System Settings > Privacy & Security > Open Anyway. Older macOS versions use System Preferences > Security & Privacy > General. Only approve the app if you trust its source. [Apple's instructions](https://support.apple.com/en-ie/102445).

## 2. Try it for ten minutes

Start with an interface-only trial, without connecting your main mailbox. To explore invented mail, quit MailCraft first, then run this in Terminal:

```sh
/Applications/MailCraft.app/Contents/MacOS/MailCraft --sample
```

The sample contains invented people and mail and keeps no mailbox changes. Explore the Inbox, search, calendar and contacts. Tell us whether the app opens and whether those screens are understandable.

If you want to test a real connection, use a dedicated test mailbox with non-sensitive data and review the [setup guide](docs/automatic-setup.md). **This is alpha software: changes made to a connected account can sync to its server.** Keep your usual mail app and avoid testing with important mail. Provider support varies; read the [known gaps](docs/gaps.md). Nobody needs to send a message or share a password to give useful first feedback.

## 3. Tell us what happened

[Open the Mac tester feedback form](https://github.com/daviddaryl-sys/mailcraft/issues/new?template=tester-signup.yml).

The most useful first report answers:

- Which Mac chip and macOS version?
- Did download, installation and first launch work?
- What was the first confusing or broken step?
- What one change would make you try it again?

GitHub issues are public. Do not post email addresses, passwords, tokens, message contents or unredacted screenshots. Report only your provider type if relevant, not your account address. There is no need to commit to daily use or connect a mailbox to participate.
