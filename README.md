<p align="center">
  <img src="assets/icons/logo-256.png" width="128" height="128" alt="SlideRelay logo">
</p>

<h1 align="center">SlideRelay</h1>

<p align="center">
  Broadcast your live PowerPoint slideshow to students' phones over the classroom Wi-Fi.<br>
  Students scan a QR code. They install nothing.
</p>

> **Status: early development.** Milestone 1 (add-in ↔ app connection) is in place.
> Capture, streaming and installers are coming next. Full documentation will follow.

## How it works

SlideRelay has two parts:

1. **PowerPoint add-in** — the **Broadcast** button on the Home tab. It's a remote control
   for the app.
2. **SlideRelay app** — a small app that runs in the menu bar (Mac) or system tray (Windows). It
   captures **only the slideshow window** (never Presenter View) and serves it to students on
   the local network.

Nothing goes to the internet. Nothing is recorded or saved. No telemetry.

## Repository layout

| Folder       | What's inside                                                         |
| ------------ | --------------------------------------------------------------------- |
| `addin/`     | Office.js add-in (TypeScript), deployed to GitHub Pages               |
| `companion/` | Tauri 2 app: Rust core in `src-tauri/`, window UIs in `ui/`           |
| `viewer/`    | The page students open in their browser                               |
| `shared/`    | Design tokens, icons and the add-in ↔ app protocol, shared by all UIs |
| `assets/`    | Logo (SVG) and every generated icon size                              |
| `docs/`      | Install, release and developer guides                                 |

## Developing

See [docs/DEVELOPING.md](docs/DEVELOPING.md).

## License

[MIT](LICENSE)
