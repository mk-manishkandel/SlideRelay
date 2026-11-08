# Changelog

All notable changes to SlideRelay are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/). The add-in and the app share one
version number.

Write entries for presenters, in plain words. Each release's notes link here.

## [Unreleased]

## [1.0.0] - 2026-11-08

The first public release.

### Added

- **Broadcast from PowerPoint:** a **Broadcast** button on the Home tab opens a panel to start,
  pause, blank and stop. The panel shows the viewer link and a QR code, and **Display QR**
  shows it large for the room.
- **Only the slideshow is shared:** SlideRelay captures the slideshow window only, never
  Presenter View, notes or other windows.
- **Viewers watch in their browser:** scan the QR code, nothing to install. Clear labels say
  "Waiting for the presenter…", "Paused by the presenter" or "Screen hidden by the presenter".
  Built for 60+ viewers; slow phones skip frames instead of falling behind.
- **Floating toolbar** during the slideshow (Pause, Blank, QR code, Stop, viewer count), on your
  own screen, never the projector. Keyboard shortcuts: Ctrl/⌃ + Alt/⌥ + P, B, Q.
- **Settings** in the panel: picture quality, updates per second, viewer PIN, network, port,
  toolbar position, start at login, quit when the broadcast stops.
- **Setup guide** on first launch: adds SlideRelay to PowerPoint and, on Mac, walks through the
  two permissions.
- **Starts quietly** in the menu bar or system tray, and quits when you stop broadcasting
  (can be turned off).
- **Automatic, signed updates**, installed only when you're not presenting.
- Windows installer (with firewall rule) and MSI; universal Mac app (Apple silicon and Intel).
- Light and dark themes, WCAG AA contrast, plain words throughout.
- No accounts, no cloud, no telemetry.

[Unreleased]: https://github.com/mk-manishkandel/SlideRelay/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/mk-manishkandel/SlideRelay/releases/tag/v1.0.0
