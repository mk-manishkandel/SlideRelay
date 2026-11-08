# Room test checklist

Run this before your first real presentation, and before publishing a release. It takes
about 15 minutes. You need: your computer with PowerPoint and SlideRelay, two phones (one on
iOS, one on Android if you can), and ideally a projector or second screen.

Use the same Wi-Fi you'll present on: networks differ.

## Setup

- [ ] SlideRelay is installed and its setup guide shows all green checks.
- [ ] PowerPoint shows **Home → Broadcast**.
- [ ] The panel connects (shows **Start Broadcast**). The first time, **Connect → Allow** worked.

## Broadcasting

- [ ] **Start Broadcast** shows a QR code and a short link.
- [ ] Both phones scan the QR code and show **"Waiting for the presenter…"**.
- [ ] Start the slideshow: both phones show the current slide within about a second.
- [ ] Change slides quickly (10 clicks): phones keep up and show the last slide.
- [ ] A slide with animation or video updates on the phones.
- [ ] The viewer count in the panel and toolbar matches the number of phones.
- [ ] Type the link by hand on a laptop browser: it works too.

## Controls

- [ ] **Pause** (toolbar or **Ctrl/⌃ + Alt/⌥ + P**): phones show "Paused by the presenter" and
      keep the slide; changing slides doesn't reach them. **Resume** catches them up.
- [ ] **Blank** (**…+ B**): phones show a black screen with "Screen hidden by the presenter".
      **Show** brings the slide back.
- [ ] **QR code** (**…+ Q**): the QR covers the projector screen. A click hides it.
- [ ] After clicking a toolbar button, the arrow keys still change slides.
- [ ] **Stop**: phones show "The broadcast has ended". SlideRelay quits (unless turned off in
      settings) and the panel says **Broadcast ended**.
- [ ] **Start a new broadcast** in the panel opens SlideRelay and starts again.

## Privacy

- [ ] With a projector or second screen and **Presenter View** on: phones show the slides,
      **never** the presenter screen with notes.
- [ ] With one screen and Presenter View on: phones don't show the notes; the panel explains
      why.
- [ ] Other windows (email, desktop) never appear on the phones, even if they cover the
      slideshow.

## Settings (panel → gear icon)

- [ ] Set a **PIN**: a new phone is asked for it; a wrong PIN is refused; the right one works.
- [ ] **Picture quality: Low** reduces sharpness; **High** makes small text sharp.
- [ ] **Toolbar position** moves the toolbar.

## Toughness

- [ ] Lock a phone for a minute, unlock: it reconnects by itself.
- [ ] Turn Wi-Fi off and on, on a phone: it reconnects.
- [ ] End the slideshow mid-broadcast: phones show "Waiting for the presenter…"; restart it:
      they're live again.
- [ ] Quit PowerPoint mid-broadcast, reopen: the panel reconnects.
- [ ] If you can, 30+ phones (a real room): the slideshow stays smooth on your computer.

Problems? Please [open an issue](https://github.com/mk-manishkandel/SlideRelay/issues/new/choose)
with the log file (see the install guides).
