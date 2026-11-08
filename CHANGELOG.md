# Changelog

All notable changes to SlideRelay are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the
project uses [Semantic Versioning](https://semver.org/). The add-in and the app share one
version number.

Write entries for presenters, in plain words. Each release's notes link here.

## [Unreleased]

## [1.0.3] - 2026-11-08

### Changed

- **Clearer status window:** the broadcast card shows the state (Live, Paused, Blanked or
  Waiting for slideshow) with a one-line explanation, the viewer count, the viewer link, and a
  simple **Stop broadcast** link.
- **Done / Finish later** is now plain text, just above the footer.

### Fixed

- **Toolbar dragging:** the toolbar no longer jumps back while you move it or when the slideshow
  starts. It stays where you put it, and opens in the same spot next time (until you change the
  toolbar position in Settings).
- **Toolbar highlights:** a button no longer stays highlighted after you click it.
- The red **Live** badge no longer shows while paused or blanked.
- PowerPoint's add-in info (the **i** in the SlideRelay panel) now shows the same version as the
  app. SlideRelay updates PowerPoint's copy of the add-in by itself after each update.

## [1.0.2] - 2026-11-08

### Added

- **Check for updates:** a link in the SlideRelay window checks right away, and
  **Install and restart** installs a downloaded update without waiting for SlideRelay to quit.

### Changed

- SlideRelay also looks for updates every 6 hours while it's running, not only when it starts.
- The setup guide's **Done** / **Finish later** button is now in the bottom row, on the right.
- **Cleaner presenting toolbar:** a coloured status pill (Live, Paused, Blank, Waiting), a drag
  handle, clearer Blank icon, and Pause / Blank stay highlighted while they're on. Nothing wraps
  onto two lines any more, and the toolbar is narrower.
- **Simpler PowerPoint panel when SlideRelay isn't open:** one button, with the extra help folded
  under "Trouble opening it?".

## [1.0.1] - 2026-11-08

### Changed

- **Faster on Windows PCs:** slides are compressed with the processor's fast (AVX2)
  instructions when available, so new slides reach phones sooner and use less CPU.
- **Lighter while presenting:** during animations and videos, SlideRelay no longer copies frames
  it would skip anyway, which saves memory and battery on both Windows and Mac.
- **One-click downloads:** the project page now has **Download for Windows** and
  **Download for Mac** buttons that always give the newest version, with install steps right
  there.

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

[Unreleased]: https://github.com/mk-manishkandel/SlideRelay/compare/v1.0.3...HEAD
[1.0.3]: https://github.com/mk-manishkandel/SlideRelay/compare/v1.0.2...v1.0.3
[1.0.2]: https://github.com/mk-manishkandel/SlideRelay/compare/v1.0.1...v1.0.2
[1.0.1]: https://github.com/mk-manishkandel/SlideRelay/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/mk-manishkandel/SlideRelay/releases/tag/v1.0.0
