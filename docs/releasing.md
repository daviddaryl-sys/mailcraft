# Releasing MailCraft

> **Last reviewed:** 2026-10-16 · **Last updated:** 2026-10-16 · **Change:** minor (the status as it is: releases published by the workflows; no riscv64 build; pinned build tools, build provenance; signing in the protected `release` environment; draft releases) · **Status:** releases v0.1.0 to v0.4.2 were built and published by `release.yml` and `package.yml` on GitHub's runners, for every platform in the table below; none is signed yet (gap #18).

The canonical recipe is [craftrules `release/playbook.md`](https://github.com/storytold/craftrules/blob/main/release/playbook.md);
this file has the MailCraft specifics. The scripts in `packaging/` are WordCraft's with MailCraft's
names, and differ from them only where the program does (what it opens, its description, its
permissions, its own installer identity), so a release has exactly the files a Craft app's has,
under the same names.

MailCraft lives outside the storytold organisation, so it cannot use the organisation's shared
release workflow (that one needs the organisation's runners and its `release` environment).
`.github/workflows/release.yml` and `package.yml` are MailCraft's own and run on GitHub's
standard runners in any public repository, a fork included. If the repository moves into the
organisation, call `storytold/craft-common-actions/.github/workflows/release.yml` instead, as
WordCraft does.

1. Bump the version: `cargo xtask version set 0.2.0` (the one source of truth is
   `[workspace.package] version`; the app shows it in `--version` and About, with the commit from
   `MAILCRAFT_BUILD_SHA`), commit, and get `cargo xtask ci` green.
2. Tag that commit `v0.2.0` and push the tag. The workflow refuses a tag that isn't the version in
   `Cargo.toml`, then builds:

| Platform | Artifacts | Signing |
|---|---|---|
| macOS | universal `MailCraft.app` in a `.dmg`, universal CLI zip | Developer ID + notarization (`APPLE_*` secrets) |
| Windows | x64, x86 and arm64 `.msi` + portable `.zip` | Azure Trusted Signing (`AZURE_*` secrets) or a certificate (`WINDOWS_CERTIFICATE*`) |
| Linux | x86_64 and aarch64 `.AppImage` (+ `.zsync` for AppImageUpdate), `.flatpak` bundle, `.deb`, `.rpm`, `.tar.gz` | checksums |
| FreeBSD | x86_64 `.tar.gz` (built in a FreeBSD VM; on a tag always, on a dry run only when asked for) | checksums |
| Web | `mailcraft-web-<version>.zip` (static site: `index.html`, wasm, glue) | none |

3. The run makes ONE **draft** GitHub release, `MailCraft v<version>` on that tag, with every file
   and a `SHA256SUMS.txt` (`sha256sum` format; `packaging/release-notes.md` is its text). Running
   the same tag again replaces the files of that draft; a release that is already published is
   never changed by the workflow (the run fails instead). A version with a suffix
   (`0.2.0-rc.1`) is marked as a pre-release.
4. A maintainer checks the draft (files, names, checksums, notes) and publishes it by hand:
   Releases › the draft › Edit › Publish release, or `gh release edit v0.2.0 --draft=false`.
   Nothing is public until then.

Signing secrets (`APPLE_*`, `WINDOWS_CERTIFICATE*` and `AZURE_*`) are secrets of the `release`
environment, not of the repository. That environment accepts version tags (`v*`) only and waits
for a maintainer's approval; the Windows and macOS jobs that sign, and the job that makes the
draft, run in it, and only for a pushed version tag. Without the secrets the builds are unsigned
(ad-hoc on macOS) and the scripts say so, so forks still build. None is set yet, so every
release so far is unsigned. There is no riscv64 Linux build, and no Flathub manifest is published.

Every file of a release also gets a build provenance attestation (`actions/attest-build-provenance`):
a signed statement, kept by GitHub outside the release, of the workflow run and commit that made
it. Check a download with `gh attestation verify <file> -R UCS-Care-Group/mailcraft`.

What the build fetches is pinned: every action by full commit SHA (Dependabot moves them, a week
after their release), the shared workflows of `storytold/craft-common-actions` by full commit SHA
(moved by hand), and every tool downloaded by URL by exact version and
sha256, checked before it is used: nfpm (`package.yml`), appimagetool and the AppImage runtime
(`packaging/linux/package.sh`), the Trusted Signing client (`packaging/windows/sign.ps1`),
rustup-init on FreeBSD and wasm-opt (`package.yml`). trunk and wasm-bindgen come pinned through
`taiki-e/install-action`, and trunk runs offline so it downloads nothing itself; the
wasm-bindgen version in `package.yml` follows the one in `Cargo.lock`. To move a tool, change
its version and its sha256 together, taken from the file itself.

Dry run: `gh workflow run release.yml --ref <branch>` builds everything on any branch, unsigned,
and keeps the files as workflow artifacts (FreeBSD only when asked for: it is slow). A dry run
names no environment, is given no secrets and publishes nothing.

## ArtCraft Launcher

The launcher installs a Craft app from its GitHub releases, finding the right file by name
(`<app>-<version>-<os>-<arch>.<ext>`) and checking it against `SHA256SUMS.txt`. What it needs of
MailCraft, and where each part stands:

| Needs | State |
|---|---|
| One release per version with Craft file names and `SHA256SUMS.txt` | The workflow above produces it; seven have been published, v0.1.0 to v0.4.1 |
| An icon in the Craft style: an engraved animal in black ink and cream on one flat app colour, in the rounded tile (`rx` 112 of 512, the figure clipped 6 units in) | `cargo xtask icon` builds every size from `assets/app-icon/source/mailcraft-art.png` once the artwork is there; until then the icon is the drawn placeholder |
| A row in the launcher's catalog (`crates/core/src/catalog.rs` in `storytold/craft-launcher`): id, name, tagline, what it is like, colours, `owner/repo`; for an app outside the organisation also its own reverse-DNS id and publisher (the "community apps" group) | A pull request on that repository, after the first release; the 128 px icon goes with it |

## What the installers do

- **Windows (`.msi`, WiX 5):** per-machine install into Program Files, a Start Menu shortcut, an
  App Paths entry, and the registrations that let the owner choose MailCraft in Settings › Default
  apps for `mailto:` links and `.eml`, `.ics` and `.vcf` files ("Open with" for `.mbox`). It never
  takes a type over from another program; only MailCraft's own `.mailcraft` opens with it by
  default. `mailcraft.exe` is a Windows program with no console window; `mailcraft-cli.exe` is the
  console program. The portable `.zip` holds the same two files and registers nothing.
- **Linux (`.deb`, `.rpm`, `.AppImage`, `.tar.gz`):** `/usr/bin/mailcraft` and `mailcraft-cli`, a
  desktop entry that offers MailCraft for `mailto:` links, messages, calendar files and contact
  cards, AppStream metadata and the icons. The binaries need glibc 2.35 or later.
- **One window per mailbox:** started a second time (the shortcut again, a link, a file from the
  file manager), MailCraft hands what it was started with to the window already open and exits
  (`apps/mailcraft/src/single_instance.rs`), so two windows never save over each other.
- **Product identity:** the MSI's UpgradeCode and component GUIDs are MailCraft's own. Never copy
  them from or into a sibling Craft app (Windows would treat the two as one product).

## Building the packages on your own machine

Windows (needs the MSVC Rust toolchain, the Windows SDK and the .NET SDK):

```
dotnet tool install --global wix --version 5.0.2
wix extension add -g WixToolset.UI.wixext/5.0.2 WixToolset.Util.wixext/5.0.2
pwsh packaging/windows/package.ps1 -Arch x64
```

Linux (build on Ubuntu 22.04 so the result runs on newer systems; these are the packages the
release workflow installs):

```
sudo apt-get install -y build-essential pkg-config libxkbcommon-dev libwayland-dev libx11-dev \
  libxrandr-dev libxi-dev libgl1-mesa-dev desktop-file-utils appstream zsync
# nfpm 2.47.0 from https://github.com/goreleaser/nfpm/releases, then:
packaging/linux/package.sh
```

Both write to `dist/release/`. Also: `packaging/macos/package.sh --arch universal`,
`packaging/linux/flatpak-bundle.sh` (after `package.sh`) and `packaging/web/package.sh`.

## What has been checked, and what has not

Checked on 2026-10-12, on a Windows 11 machine and an Ubuntu 22.04 WSL distribution:

- `package.ps1 -Arch x64` builds `mailcraft-<version>-windows-x64.msi` and the portable zip; the
  MSI was read back (identity, files, shortcut, registrations) and unpacked; the program in it is
  a GUI program and the CLI answers `--version`.
- `package.sh` builds the AppImage (with `.zsync`), `.deb`, `.rpm` and `.tar.gz`; the `.deb` was
  installed on a bare system, the CLI ran on the sample mailbox, the window started under a
  virtual X server, answered the control channel and drew itself, and the package uninstalled.
- Found and fixed on the way: the MSI carried WordCraft's UpgradeCode and component GUID; on X11
  the window needs `libxkbcommon-x11`, which the packages did not ask for; the desktop entry
  could not receive a `mailto:` link (`%F`) and named two main categories.

Since then, on GitHub's runners with every release: the x64 MSI is installed, checked and
uninstalled, the x86_64 and aarch64 `.deb` installed, started and removed, and the `.dmg` checked;
the x86 and arm64 Windows builds, the Flatpak bundles and the FreeBSD tarball are built.

Not checked yet: the x86 and arm64 Windows installers, the Flatpak and the FreeBSD tarball
installed and run on a machine of their own; macOS on a real Mac (and it has no DMG artwork yet:
`packaging/macos/dmg/README.md`); signing. On
macOS, `mailto:` links are not handed to the program yet (no `CFBundleURLTypes`). On Linux a
sign-in remembered in the Secret Service across a restart is still to be seen on GNOME and KDE,
and in the Flatpak (which is granted `org.freedesktop.secrets` for it).

## Revision history

| Date | Change |
|---|---|
| 2026-10-16 | Status as it is: v0.1.0 to v0.4.1 published by `release.yml` and `package.yml`, unsigned; riscv64 and the Flathub manifest out of the table (neither is built); what the runners check on every release |
| 2026-10-16 | Signing secrets in the protected `release` environment, used only for a pushed version tag; dry runs unsigned with no secrets; the workflow makes a draft that a maintainer publishes |
| 2026-10-16 | Pinned build tools (version and sha256) and build provenance for every release file |
| 2026-10-15 | Linux packages: the Secret Service for remembered sign-ins (the Flatpak may talk to `org.freedesktop.secrets`; the .deb and .rpm suggest a keyring); what is still to be checked |
| 2026-10-12 | First local builds of the Windows and Linux packages; installer identity, file types and `mailto:`, one window per mailbox, X11 dependency; what is and is not checked |
| 2026-10-12 | A release workflow of MailCraft's own (tag-triggered, standard runners), FreeBSD script, release notes; what ArtCraft Launcher needs |
| 2026-10-11 | First version |
