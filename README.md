<p align="center">
  <img src="assets/icons/logo-256.png" width="112" height="112" alt="SlideRelay logo">
</p>

<h1 align="center">SlideRelay</h1>

<p align="center">
  <strong>Show your live PowerPoint slideshow on everyone's phone, over the room's Wi-Fi.</strong><br>
  Viewers scan a QR code and watch in their browser. They install nothing.
</p>

<p align="center">
  <a href="https://github.com/mk-manishkandel/SlideRelay/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/mk-manishkandel/SlideRelay?label=download&color=2563EB"></a>
  <a href="https://github.com/mk-manishkandel/SlideRelay/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/mk-manishkandel/SlideRelay/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-2563EB"></a>
  <img alt="Windows and macOS" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS-555">
</p>

## Download

<p align="center">
  <a href="https://github.com/mk-manishkandel/SlideRelay/releases/latest/download/SlideRelay-Windows-setup.exe"><img alt="Download for Windows" src="https://img.shields.io/github/v/release/mk-manishkandel/SlideRelay?style=for-the-badge&color=2563EB&labelColor=1E40AF&label=%E2%AC%87%20Download%20for%20Windows"></a>
  &nbsp;
  <a href="https://github.com/mk-manishkandel/SlideRelay/releases/latest/download/SlideRelay-Mac.dmg"><img alt="Download for Mac" src="https://img.shields.io/github/v/release/mk-manishkandel/SlideRelay?style=for-the-badge&color=2563EB&labelColor=1E40AF&label=%E2%AC%87%20Download%20for%20Mac"></a>
</p>

<p align="center">
  Windows 10 or 11 (64-bit) · macOS 13 or later (Apple silicon and Intel)<br>
  Always the newest version · Free · No account<br>
  <a href="https://github.com/mk-manishkandel/SlideRelay/releases/latest">All files, checksums and release notes</a>
</p>

You also need the **PowerPoint desktop app** for Windows or Mac (Microsoft 365 recommended).
Viewers just need a phone, tablet or laptop with a browser, on the same Wi-Fi.

<details>
<summary><strong>Install on Windows</strong> (about 3 minutes)</summary>

1. Click **Download for Windows**. If your browser says the file _"isn't commonly downloaded"_,
   keep it: in Edge, **⋯ → Keep → Show more → Keep anyway**; in Chrome, **Keep**. This shows
   because SlideRelay is new and not yet signed with a paid certificate. It isn't a virus warning.
2. Open **SlideRelay-Windows-setup.exe**. If you see _"Windows protected your PC"_, click
   **More info → Run anyway**.
3. Choose **Anyone who uses this computer** if you can (it also lets phones through the firewall),
   then **Install**.
4. In SlideRelay's setup guide, click **Add to PowerPoint**, then **Done**.
5. Quit and reopen PowerPoint. You'll find **Broadcast** on the **Home** tab.

IT departments: an [MSI package](https://github.com/mk-manishkandel/SlideRelay/releases/latest/download/SlideRelay-Windows.msi)
is available. Full guide and troubleshooting: [docs/INSTALL-WINDOWS.md](docs/INSTALL-WINDOWS.md).

</details>

<details>
<summary><strong>Install on Mac</strong> (about 3 minutes)</summary>

1. Click **Download for Mac**, open **SlideRelay-Mac.dmg**, and drag **SlideRelay** to
   **Applications**.
2. Open SlideRelay. The first time, macOS says it can't verify the app (SlideRelay isn't notarized
   by Apple yet). Click **Done**, then go to **System Settings → Privacy & Security** and click
   **Open Anyway**. You only do this once.
3. In SlideRelay's setup guide:
   - **Add to PowerPoint** (allow access if macOS asks),
   - **Let PowerPoint connect** (enter your Mac password once),
   - **Allow Screen Recording** (switch SlideRelay on),
   - then click **Done**. SlideRelay lives in the menu bar.
4. Quit PowerPoint (⌘Q) and reopen it. You'll find **Broadcast** on the **Home** tab.

Full guide and troubleshooting: [docs/INSTALL-MAC.md](docs/INSTALL-MAC.md).

</details>

> Download SlideRelay only from this GitHub page. It's free and has no other official download site.

## How it works

1. Install SlideRelay. On first launch, its setup guide adds a **Broadcast** button to
   PowerPoint's **Home** tab.
2. In PowerPoint, click **Home → Broadcast → Start Broadcast**. A QR code appears.
3. Viewers scan the QR code. When you start your slideshow, they see it live, usually within a second.

While you present, a small toolbar lets you **pause** (viewers keep the current slide),
**blank** the screen, show the **QR code** on the projector, or **stop**. Viewers always see a
short label saying what's happening.

| Feature                  |                                                                                                                   |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| Only the slideshow       | SlideRelay captures **only the slideshow window**. It never shows Presenter View, your notes, or other windows.   |
| Many viewers             | Built for a full room: automated tests run 60 viewers at once. Slow phones skip frames instead of falling behind. |
| No accounts, no internet | Everything stays on your local network. Nothing is uploaded, recorded or saved.                                   |
| Optional PIN             | Ask viewers for a 4–8 digit PIN before they can watch.                                                            |
| Read-only                | Viewers can only watch. They can't control anything.                                                              |
| Light and dark           | Follows your system theme. Built to WCAG AA contrast.                                                             |

## Can I trust it?

You're installing software that sees your screen, so you should ask. Here is how you can check:

- **Open source.** All the code is here, under the [MIT License](LICENSE). Anyone can read it,
  build it and check it.
- **Built in public.** Every installer is built by [GitHub Actions](.github/workflows/release.yml)
  straight from the tagged source code, not on someone's laptop. Each release lists the exact
  build that made it.
- **Checksums.** Each release has a `SHA256SUMS.txt` file. To check your download:
  - Windows (PowerShell): `Get-FileHash "$env:USERPROFILE\Downloads\SlideRelay-Windows-setup.exe"`
  - Mac (Terminal): `shasum -a 256 ~/Downloads/SlideRelay-Mac.dmg`

  The result must match the line for that file in `SHA256SUMS.txt`.

- **Signed updates.** SlideRelay only installs updates signed with this project's private key.
  A tampered or older update is refused.
- **No telemetry.** SlideRelay collects nothing and sends nothing about you. See
  [PRIVACY.md](PRIVACY.md) for exactly what goes over the network.
- **Narrow permissions.** On a Mac it asks for Screen Recording and uses it only for the
  slideshow window. The control connection to PowerPoint only listens on this computer and
  needs a one-time approval.

**Why does Windows or macOS warn me?** Removing those warnings needs paid certificates
from Microsoft and Apple. As a free project, SlideRelay doesn't have them yet; Windows signing
through the free [SignPath Foundation](https://signpath.org) programme is in progress
([docs/SIGNING.md](docs/SIGNING.md)). Until then, Windows shows "Windows protected your PC" and
macOS says it "could not verify" the app. The install steps above show exactly what to click,
and why it's safe when you downloaded from this page.

Found a security problem? Please report it privately: see [SECURITY.md](SECURITY.md).

## Documentation

|                                                                                                     |                                                               |
| --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------- |
| [Install on Windows](docs/INSTALL-WINDOWS.md)                                                       | Download, install, firewall, first broadcast, troubleshooting |
| [Install on Mac](docs/INSTALL-MAC.md)                                                               | Download, first open, permissions, troubleshooting            |
| [Room test checklist](docs/TEST-CHECKLIST.md)                                                       | Try everything before your first real presentation            |
| [Deploying for an organisation](docs/APPSOURCE.md)                                                  | Install the add-in for everyone via Microsoft 365             |
| [Privacy](PRIVACY.md) · [Security](SECURITY.md)                                                     | What SlideRelay does and doesn't do                           |
| [Developing](docs/DEVELOPING.md) · [Releasing](docs/RELEASING.md) · [Contributing](CONTRIBUTING.md) | For contributors                                              |

## How it's built

SlideRelay has two parts:

1. **PowerPoint add-in** (TypeScript, Office.js): the **Broadcast** button and panel. It's a
   remote control for the app, served from GitHub Pages, so it updates by itself.
2. **SlideRelay app** (Tauri 2: Rust core, small web UI): runs in the menu bar or system tray.
   It captures the slideshow window with ScreenCaptureKit (Mac) or Windows.Graphics.Capture
   (Windows), encodes it as JPEG and streams it over WebSocket to viewers on the local network.

| Folder       | What's inside                                           |
| ------------ | ------------------------------------------------------- |
| `addin/`     | PowerPoint add-in, deployed to GitHub Pages             |
| `companion/` | The app: Rust core in `src-tauri/`, window UIs in `ui/` |
| `viewer/`    | The page viewers open in their browser                  |
| `shared/`    | Design tokens, icons and the add-in ↔ app protocol      |
| `assets/`    | Logo and every icon size                                |
| `docs/`      | Install, release and developer guides                   |

## License

[MIT](LICENSE) © Manish Kandel and SlideRelay contributors.
PowerPoint and Microsoft 365 are trademarks of Microsoft. SlideRelay is an independent
project, not made or endorsed by Microsoft.
