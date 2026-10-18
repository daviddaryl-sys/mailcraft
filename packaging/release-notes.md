**What's new in 0.4.3** (an alpha for testers; sign up in [#11](https://github.com/GITHUB_REPOSITORY/issues/11)):
- Microsoft 365 and Outlook.com accounts move from IMAP to Microsoft Graph. An account set up with an earlier version is offered the switch once (Switch, Later): what waits to be sent goes first, the mailbox is backed up beside its file, and the mail comes down again through Graph with folders kept. New Microsoft accounts use Graph from the start.
- Signing in as someone else is no longer taken silently. If Microsoft's page signs in a different account than the address typed, Add Account asks: sign in again, or set up the account that signed in. An existing account refuses a sign-in that isn't its own and says so.
- Told when a new version is out, if you want it: MailCraft asks once whether to check GitHub daily (off until you say yes; nothing about you or your mail is sent), and File › About has Check for Updates. Nothing is downloaded or installed.
- Reply All: a long To line can be clicked at its end again (the Bcc link sat over it), so addresses there can be deleted and the message body takes typing afterwards.
- The flag and delete buttons on a message in the list are bigger and further apart, and the attachment mark keeps clear of them.
- Send/Receive no longer stops with "AccessTokenExpired" on a Microsoft account: the sign-in is renewed before it would run out mid-run.
- Preferences (`ui.json`) are saved whole even if MailCraft is stopped mid-save, and a damaged file is set aside as `ui.json.damaged` instead of being overwritten.
- macOS: quitting no longer risks a crash in the system's Touch Bar code; MailCraft quits through the system, asking first about a message being written.
- Release pipeline hardening: every file now comes with a build provenance attestation (`gh attestation verify <file> -R GITHUB_REPOSITORY`).

**Early alpha.** The foundations are in place and tested, but much of what a daily mail program needs is not there yet, and most of the Microsoft 365 behaviour has only been tested against stand-ins. What works is in the [README](https://github.com/GITHUB_REPOSITORY#what-works-today).

MailCraft for Windows, macOS, Linux, FreeBSD and the web. Every file is listed with its SHA-256 in `SHA256SUMS.txt`.

| Platform | File | |
|---|---|---|
| Windows 10/11 (x64, x86, ARM64) | `mailcraft-<ver>-windows-<arch>.msi` | Installer for all users: Program Files\MailCraft and the Start menu; offers MailCraft for `mailto:` links and for `.eml`, `.ics` and `.vcf` files without taking them over (choose it in Settings › Default apps); uninstall from Installed apps. `mailcraft-cli.exe` is installed next to the app. |
| | `mailcraft-<ver>-windows-<arch>-portable.zip` | Portable: unzip anywhere and run `mailcraft.exe`; includes `mailcraft-cli.exe`. Registers nothing. |
| macOS 11+ (Apple silicon and Intel) | `mailcraft-<ver>-macos-universal.dmg` | Drag MailCraft to Applications. |
| | `mailcraft-cli-<ver>-macos-universal.zip` | The command-line tool and MCP server alone. |
| Linux x86_64 and aarch64 (glibc 2.35+) | `mailcraft-<ver>-linux-<arch>.AppImage` | `chmod +x` and run (if it does not start, add `--appimage-extract-and-run`). The `.zsync` next to it enables delta updates. |
| | `mailcraft-<ver>-linux-<arch>.deb` / `.rpm` | `sudo apt install ./mailcraft-*.deb` or `sudo dnf install ./mailcraft-*.rpm`. |
| | `mailcraft-<ver>-linux-<arch>.flatpak` | `flatpak install --user mailcraft-*.flatpak` (the freedesktop runtime comes from Flathub). |
| | `mailcraft-<ver>-linux-<arch>.tar.gz` | `bin/mailcraft`, `bin/mailcraft-cli`, the desktop entry, metainfo and icons under `share/`. |
| FreeBSD 15 (x86_64) | `mailcraft-<ver>-freebsd-x86_64.tar.gz` | The two programs, the desktop entry and the icons. |
| Any modern browser | `mailcraft-web-<ver>.zip` | The app as a static site (WebAssembly). It cannot reach mail servers from a browser: bring mail in with File › Import. |

MailCraft keeps your mailbox in its settings folder and saves as you go; one window per mailbox. With a Microsoft 365 or Outlook.com account you sign in on Microsoft's page, and the sign-in is kept by the system's credential store (Windows, macOS, and the Secret Service of GNOME Keyring or KWallet on Linux). What works, and how far each part is from Outlook, is in the [README](https://github.com/GITHUB_REPOSITORY#what-works-today) and in `docs/gaps.md`.

The builds are not signed with a publisher certificate yet: Windows SmartScreen and macOS Gatekeeper will ask before running them.
