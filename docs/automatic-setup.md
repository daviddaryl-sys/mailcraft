# Automatic account setup in the local MailCraft build

Add Account asks for an email address, then a password or the provider's own sign-in. A known
provider's settings take priority over DNS guesses. Despres Connect has a local profile for
`despresai.com` and `badmintonsoldiers.club`: encrypted VM IMAP at
`outlook.despresai.com:9933`, each domain's existing `mail.<domain>:587` SMTP relay with mandatory
STARTTLS, and the shared `https://outlook-test.computerhelpexpert.co.uk/dav/` endpoint for both
calendar and contacts. It never selects the retained Plesk IMAP service on port 993.
For Badminton Soldiers, enter the full email address and current migrated-mailbox password;
no server fields or separate DAV sign-in are required. The known profile takes priority over the
domain's legacy MX records. Matching is case-insensitive and limited to these exact domains.
The mailbox must have IMAP, SMTP and DAV access enabled on the VM. An already connected
account is preserved; this update does not remove it, duplicate it or change its settings.

One password is handed in memory to the mail authentication check and read-only DAV collection
discovery. Both checks must succeed when DAV is configured. No message, test appointment or
contact is sent. The account is then saved, the password kept using the existing macOS Keychain
preference, and mail plus an initial read-only calendar/contact download start automatically.
The completion screen shows DAV progress/errors separately from mail. An existing account is
detected before sign-in, rather than duplicated. The current account needs no removal or re-add.

For any other domain, MailCraft first asks the fixed HTTPS endpoint
`https://outlook.despresai.com/.well-known/mailcraft?domain=<domain>`. It sends only the domain:
no mailbox local part, password, cookie or authorization header. The API performs an exact,
read-only check against active VM domains; it does not publish a domain list or any users.
A positive response must name that exact domain, the expected provider and schema version.
It enables the built-in Despres Connect profile (VM IMAP 9933, central
`mail.despresai.com:587` STARTTLS relay and shared DAV). No endpoints are accepted from the
response, and no redirects are followed. The lookup has a four-second timeout and 1 KiB body
limit. Newly active VM domains need no app rebuild; inactive, unknown, invalid or unconfirmed
domains do not receive this profile. Account protocol access and outbound relay provisioning
remain part of normal server onboarding. A domain match is not a mailbox authentication check:
mail and read-only DAV authentication must still succeed before adding an account.

Existing exact profiles for the two already verified migrated domains are retained. Known public
providers (including Microsoft and iCloud) keep their existing setup and are not queried here.
If Despres Connect does not confirm an unfamiliar domain, its normal provider/DNS/manual flow
continues. Generic DNS lookups use domain-only DNS-over-HTTPS for MX and encrypted service records:
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
when those endpoints change. AUTO.3 adds the domain-only API route and a restricted HTTPS ingress location. It changes no
mailbox privilege, password, SMTP route, DNS record, mail service or ActiveSync setting. The
corresponding source download includes the API update and deployment instructions.

Primary references: [RFC 6764](https://www.rfc-editor.org/rfc/rfc6764.html) (DAV discovery),
[RFC 6186](https://www.rfc-editor.org/rfc/rfc6186.html) (mail service discovery),
[RFC 8314](https://www.rfc-editor.org/rfc/rfc8314.html) (encrypted mail services).
