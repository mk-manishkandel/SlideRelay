# Privacy

**Short version:** SlideRelay collects nothing about you or your viewers. There are no
accounts, no analytics, no telemetry, and no cloud servers. Your slides never leave your
local network.

## What happens on your computer

- The SlideRelay app captures **only the PowerPoint slideshow window** while you broadcast,
  and sends it to viewers on your local network. Nothing is recorded or saved to disk.
- It saves your settings and approved-connection tokens (hashed) in its own folder, and a
  log file (kept 7 days) for troubleshooting. Logs contain technical events, never slide
  content. They stay on your computer unless you attach them to a bug report.

## What goes over the network

| Connection                                                             | Why                                                   | What's sent                                           |
| ---------------------------------------------------------------------- | ----------------------------------------------------- | ----------------------------------------------------- |
| Viewers' browsers → your computer (local network only)                 | To watch the broadcast                                | Slide images, and the PIN if you set one              |
| PowerPoint panel → SlideRelay app (this computer only)                 | Remote control                                        | Commands like Start and Pause                         |
| PowerPoint → `mk-manishkandel.github.io` and Microsoft's Office.js CDN | Loads the add-in's panel                              | Standard web requests (your IP address, browser info) |
| SlideRelay app → `github.com`                                          | Checks for updates, about once per start, when online | A standard web request (your IP address, app version) |

GitHub and Microsoft have their own privacy policies for those web requests. SlideRelay
adds nothing to them.

## Viewers

Viewers don't sign in or install anything. Their browser connects to your computer on the
local network. SlideRelay counts how many are connected (shown to you as a number) but doesn't
store who they are.

## Questions

Open an [issue](https://github.com/mk-manishkandel/SlideRelay/issues), or email
**hello@manishkandel.com.np**.
