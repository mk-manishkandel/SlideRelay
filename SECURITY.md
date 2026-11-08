# Security policy

SlideRelay sees a presenter's screen and serves it on a local network, so we take security
seriously. Thank you for helping keep its users safe.

## Reporting a vulnerability

**Please don't open a public issue.** Report it privately instead:

- [**Report a vulnerability**](https://github.com/mk-manishkandel/SlideRelay/security/advisories/new)
  with GitHub's private reporting (only the maintainers can see it), or
- email **hello@manishkandel.com.np** with "SlideRelay security" in the subject.

Include what you found, how to reproduce it, and what an attacker could do. You'll get a
reply within 7 days. We'll agree a disclosure date with you (normally within 90 days) and
credit you in the release notes unless you prefer not to be named.

## Supported versions

Only the [latest release](https://github.com/mk-manishkandel/SlideRelay/releases/latest) gets
security fixes. SlideRelay updates itself, so most users are on it.

## How SlideRelay is designed to be safe

- **Only the slideshow window is captured**, never Presenter View, notes, or other windows.
- **Viewers can only watch.** The viewer server has no way to control anything; each broadcast
  has a new random link code, and an optional PIN (with limited attempts).
- **The PowerPoint control API listens only on this computer** (`127.0.0.1`), checks the
  `Host` and `Origin` headers (blocking DNS-rebinding and other websites), and needs a token
  the presenter approves once. Tokens are stored hashed.
- **Updates are signed** and checked against the signed version (no downgrades).
- **No telemetry, no accounts, no cloud.** See [PRIVACY.md](PRIVACY.md).
- **Minimal permissions** for the app's own windows (no file system or shell access from
  the UI) and `Restricted` permissions for the add-in (it can't read your documents).

### Known limitations

- The viewer stream is plain HTTP on the local network (browsers can't trust a certificate
  for a local IP address without setup on every phone). Anyone on the same network who has
  the link (or PIN) can watch, and someone monitoring that network could see the slides.
  Don't broadcast confidential slides on untrusted networks.
- Installers aren't signed by Microsoft- or Apple-trusted certificates yet; see
  [docs/SIGNING.md](docs/SIGNING.md). Verify downloads with `SHA256SUMS.txt`.
