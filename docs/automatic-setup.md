# Automatic account setup in the local MailCraft build

Add Account asks for an email address, then a password or the provider's own sign-in. A known
provider's settings take priority over DNS guesses. Despres Connect has a local profile for
`despresai.com`: encrypted IMAP on port 9933, the existing SMTP relay with mandatory STARTTLS,
and the shared DAV endpoint. It never selects the legacy Plesk IMAP service on port 993.

One password is handed in memory to the mail authentication check and read-only DAV collection
discovery. Both checks must succeed when DAV is configured. No message, test appointment or
contact is sent. The account is then saved, the password kept using the existing macOS Keychain
preference, and mail plus an initial read-only calendar/contact download start automatically.
The completion screen shows DAV progress/errors separately from mail. An existing account is
detected before sign-in, rather than duplicated. The current account needs no removal or re-add.

Unknown domains use domain-only DNS-over-HTTPS lookups for MX and encrypted service records:
`_imaps._tcp`, `_submissions._tcp` (preferred over `_submission._tcp` with mandatory STARTTLS),
`_caldavs._tcp` and `_carddavs._tcp`. DAV services can publish TXT `path` records naming shared or separate
HTTPS contexts. Known iCloud profiles use Apple’s advertised service roots without requiring TXT paths. Lower SRV priority wins, with weighted choice at equal priority. Lookups are
bounded and run on background workers; only the domain goes to DNS, never an email local part
or credential. Cancellation discards the result and never starts a sign-in.

iCloud addresses (`icloud.com`, `me.com`, `mac.com`), iCloud selected manually and iCloud MX
recognition now carry both Apple DAV roots. Use an app-specific password generated in the Apple
Account security settings; two-factor authentication is required. An optional DAV sign-in field
handles a different Apple Account login. See [icloud-setup.md](icloud-setup.md).

Automatic setup is not universal. Generic well-known redirects and arbitrary cross-origin DAV
hrefs remain refused. Missing DNS paths and unsupported provider behavior keep the manual setup
path. Existing Microsoft OAuth is preserved. This feature does not add meetings, tasks or
ActiveSync/MAPI. Existing DAV limits remain in [dav-sync.md](dav-sync.md).

The Despres profile describes the deployment verified on 17 October 2026; update it deliberately
when those endpoints change. This feature changes the local client, with no DNS/server changes.

Primary references: [RFC 6764](https://www.rfc-editor.org/rfc/rfc6764.html) (DAV discovery),
[RFC 6186](https://www.rfc-editor.org/rfc/rfc6186.html) (mail service discovery),
[RFC 8314](https://www.rfc-editor.org/rfc/rfc8314.html) (encrypted mail services).
