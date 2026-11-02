# Email design

All ten email variants use `mailer/templates/layout.html` through
`render_email_layout` in `mailer/templates.rs`. Individual HTML templates are
body fragments, not full documents. The footer stays inside the same responsive
container as the message.

The palette follows the mobile client's `lib/core/theme/app_colors.dart`:

| Role | Cream default | Dark preference |
| --- | --- | --- |
| Background | `#F1ECE2` | `#0D0D0F` |
| Paper | `#FBF9F3` | `#161618` |
| Excerpt | `#ECE6D9` | `#212124` |
| Main text | `#211F1A` | `#ECECE4` |
| Secondary text | `#5B5648` | `#B1AEA6` |
| Link/accent ink | `#08134F` | `#C7BCA6` |
| Button fill | `#08134F` | `#08134F` |

Mobile uses Lora; emails use Georgia/Times as a broadly available serif fallback,
without fetching web fonts. The logo is the existing mobile PNG, copied unchanged
to `public/assets/email-icon.png`; a text wordmark remains when images are blocked.

Cream defaults are inline for clients that strip stylesheets. The full document
declares light/dark color schemes and supplies progressive dark styles using
`prefers-color-scheme`. This uses the mailbox's supported preference, not the
user's app setting, and does not change any saved mobile theme preference.
Some clients ignore the query or rewrite/invert colors themselves; exact matching
of all mailbox themes is not guaranteed. See [Gmail CSS support](https://developers.google.com/workspace/gmail/design/css)
and [dark-mode compatibility](https://www.caniemail.com/features/css-at-media-prefers-color-scheme/).

Presentation tables, a 600px maximum width, an Outlook desktop width fallback and
mobile padding keep the layout simple. No shadows, tracking pixels or JavaScript.
Buttons and fallback URLs are real links; the password-reset message retains its
30-minute expiry and raw-URL fallback. Link destinations, recipients, notification
rules and plain-text alternatives are unchanged.

Dynamic fields remain HTML-escaped. Excerpts preserve explicit newlines. Template
substitution is single-pass so placeholder-like user text is not interpreted as
another field. The optional screenshot gallery contains synthetic data only.

## Local previews and checks

Run the focused tests without a database, SMTP service or production credentials:

```bash
cargo test --lib mailer::templates
```

To export all ten standalone HTML previews, create a private temporary directory
and pass it explicitly:

```bash
preview_dir=$(mktemp -d /tmp/hupo-email-preview.XXXXXXXX)
HUPO_EMAIL_PREVIEW_DIR="$preview_dir" cargo test --lib renders_all_email_variants_with_mobile_cream_defaults
```

Open the HTML files in a browser. Their logo is embedded for offline preview; real
emails link to the public PNG. Browser theme/device emulation can check the CSS,
but cannot reproduce Gmail's or Outlook's HTML transformations. Test representative
emails in actual light/dark inboxes before treating client compatibility as verified.
The preview process never sends mail or reads user records.

No migration or new environment variable is needed. Existing `API_URL` supplies
the logo URL and `APP_BASE_URL` supplies the existing links. Deploy the public PNG
with the backend. Already-sent and already-rendered queued emails keep their old
HTML; only newly generated emails receive the redesign.
