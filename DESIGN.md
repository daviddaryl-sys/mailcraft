# MailCraft desktop design

## Direction

An inbox for someone scanning several accounts on a large monitor in daytime light, then returning to the same work after dark. Keep the window steady so their eyes can move from folder to subject to reply without relearning the layout.

Use a restrained teal and mint palette drawn from the Despres Connect envelope. Quiet green-tinted neutrals separate chrome, folders and content. In the dark appearance, increase the separation between the text and the surrounding surfaces.

| Role | Light | Dark |
|---|---|---|
| Window | `#EAF0EF` | `#13201F` |
| Sidebar | `#EDF3F1` | `#172623` |
| Message and reading surface | `#FAFCFB` | `#222826` |
| Main ink | `#192D2A` | `#E6F1ED` |
| Secondary ink | `#4C655F` | `#B5C8C1` |
| Primary action and focus | `#096758` | `#88DBBF` |
| Selection background | `#D9ECE5` | `#294940` |

The native egui tokens in `crates/ui-egui/src/theme.rs` are the source of truth and use sRGB channel values. Keep warning, follow-up, and category colors meaningful and independent from the theme accent.

## Typography and layout

Use the app's existing UI font stack. The active folder and selected message subject anchor the workspace; sender, subject, preview and time follow a stable grid. Date groups are quiet and full width. Give long text the existing wrapping or clipping behaviour, with the folder's full title on hover. In narrower windows give the reading pane the larger share, move the message actions below the subject, and move the date below the sender. Reading, folder and message list panes retain their resizing and visibility controls.

## Components and interaction

- New Email is the filled primary action on the simplified ribbon. Its adjacent arrow still opens the New Items menu.
- The folder pane opens Add Mailbox from a small, labelled plus. The existing account setup dialog and File entry remain available.
- A filled dot plus heavier text marks unread messages. Its click target toggles read state without selecting the message.
- Selected rows use a tint; the keyboard target also has an outline. Follow-up flags and overdue labels keep their independent meaning.
- No decorative transitions. Check the actual native window with sample mail in both appearances and at a narrower window width.
