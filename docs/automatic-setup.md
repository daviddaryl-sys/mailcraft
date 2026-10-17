# Automatic account setup in the Despres Connect build

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
`_caldavs._tcp` and `_carddavs._tcp`. DAV services must publish TXT `path` records naming the same
HTTPS context. Lower SRV priority wins, with weighted choice at equal priority. Lookups are
bounded and run on background workers; only the domain goes to DNS, never an email local part
or credential. Cancellation discards the result and never starts a sign-in.

This version does not promise automatic setup for every provider. Separate calendar/contact
origins, redirects through well-known endpoints, DAV providers without TXT paths and iCloud's
service-specific authentication/discovery need additional support. Unknown/partial settings
retain the manual setup path, and mail-only setup is identified on completion. Existing Microsoft
OAuth handling is preserved. Automatic setup does not add meeting scheduling or tasks, nor
ActiveSync/MAPI compatibility. Existing DAV limitations remain in [dav-sync.md](dav-sync.md).

The Despres profile describes the deployment verified on 17 October 2026; update it deliberately
when those endpoints change. This feature changes the local client, with no DNS/server changes.

Primary references: [RFC 6764](https://www.rfc-editor.org/rfc/rfc6764.html) (DAV discovery),
[RFC 6186](https://www.rfc-editor.org/rfc/rfc6186.html) (mail service discovery),
[RFC 8314](https://www.rfc-editor.org/rfc/rfc8314.html) (encrypted mail services).
