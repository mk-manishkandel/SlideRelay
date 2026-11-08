# Build SlideRelay — an open-source PowerPoint live-broadcast add-in (Windows + macOS)

You are building **SlideRelay** from scratch: an open-source Microsoft PowerPoint add-in for **Windows and macOS**. Build the complete, working project, following Microsoft's Office add-in guidelines, platform guidelines for Windows and macOS, and open-source best practices.

I am a beginner developer and will maintain this project myself using plain git and GitHub, so keep the code clean, well-commented and well-documented.

---

## 1. What it does

Teachers use SlideRelay in classrooms where students can't clearly see the projector. With one click, the teacher broadcasts the live slideshow over the local Wi-Fi. Students scan a QR code (or type a short link) and watch the presentation live in their phone or laptop browser. Students install nothing.

Students must see **exactly what the projector shows** (the audience slideshow window, with animations and transitions) and **never the Presenter View**.

---

## 2. Tech stack (decided — use this)

- **PowerPoint add-in:** Office.js, written in TypeScript. Works in desktop PowerPoint on Windows and Mac (Microsoft 365 and PowerPoint 2016+ where supported).
- **Companion app:** Tauri 2, with **Rust** for the core (window capture, frame encoding, web server, session logic) and **TypeScript** for its UI windows (QR window, floating toolbar, setup wizard, settings).
- **Window capture:** Windows.Graphics.Capture on Windows; ScreenCaptureKit on macOS. Use well-maintained Rust crates where available, with native bindings where needed.
- **Student viewer page:** plain TypeScript + HTML/CSS (no heavy framework), served by the companion app, receiving frames over WebSocket.
- Use the same TypeScript tooling (e.g., Vite) across the add-in, companion UI and viewer.

### Why two parts
Desktop PowerPoint on Mac does not support COM/VSTO add-ins, and Office.js add-ins cannot capture windows or run a network server. So:

1. **Office.js add-in** — the ribbon button and task pane inside PowerPoint. Talks to the companion app over an authenticated localhost connection. Shows companion status; if it isn't running, shows "SlideRelay app is not running" with a launch/download link. Warns when the companion app is outdated or incompatible (use a versioned API between them).
2. **Tauri companion app** — a small background/tray app that does capture, streaming, the QR window, the floating toolbar and the setup wizard. Auto-start option.

Solve the HTTPS add-in page ↔ localhost companion connection so it works reliably in the Office webviews on both Windows and Mac.

---

## 3. Core requirements

### 3.1 Mirror exactly the audience slideshow window, never the Presenter View
- Capture ONLY the slideshow (audience) window, exactly as it appears on the projector, including animations, transitions, embedded video, and pen/laser annotations.
- The Presenter View window (notes, next-slide preview, timer) must NEVER be captured or sent.
- Use per-window capture, not full-screen capture.
- Must work on a single monitor, with a projector in extended mode, and with Presenter View on or off, on both Windows and Mac.
- Automatically detect when the slideshow starts and ends, and re-attach if the slideshow window is recreated.

### 3.2 Local network streaming
- The companion app runs a lightweight web server serving the viewer page on the local network (e.g., `http://<LAN-IP>:<port>/<session-code>`).
- No internet or cloud required; everything stays on the LAN.
- Low latency (target under 1 second) and must handle 60+ simultaneous student devices.
- Encode each frame once (JPEG or WebP) and fan it out to all viewers over WebSocket. Send frames only when the content changes. Drop stale frames for slow clients instead of queuing them.
- Pick the correct LAN IP automatically (ignore virtual/VPN adapters); let the teacher change it if needed.
- Handle the Windows Firewall rule and the macOS firewall/Local Network permission cleanly, preferably during install, so the teacher doesn't need admin rights at broadcast time.

### 3.3 QR code & link
- When the broadcast starts, show a window with a large QR code and the short URL.
- Include a "Show QR on projector" option so the whole class can scan it.

### 3.4 Floating control toolbar
- While broadcasting, show a small always-on-top floating toolbar on the teacher's screen with:
  - **Pause** — students' view freezes on the last frame.
  - **Resume** — live view continues.
  - **Blank** — students see a black screen.
  - **Show QR**
  - **Stop** — ends the broadcast and invalidates the link.
- Show the live count of connected viewers.
- The toolbar must never appear in the broadcast.
- Keyboard shortcuts for Pause, Blank and Stop (Windows and Mac conventions).
- Pause / Blank / Stop must also be available from the add-in's task pane.

### 3.5 Security & privacy
- Each session gets a random session code in the URL; old links stop working after Stop.
- Optional PIN for viewers.
- Viewers are strictly read-only.
- The localhost link between the add-in and the companion app must be authenticated so other software can't control it.
- No telemetry or analytics. Nothing is recorded or saved.

### 3.6 Settings
- Port, stream quality/frame rate, network adapter, optional PIN, toolbar position.

---

## 4. UI / UX (high priority)

The whole product must feel **very clean, simple, elegant and easy to use**. Teachers are not technical and will use it in front of a class, so every interaction must be obvious, calm and fast.

### Principles
- One obvious action per screen. Starting a broadcast must take one click.
- Minimal and quiet: plenty of white space, one accent color (from the logo), one clean sans-serif font, soft rounded corners, subtle shadows. No clutter, gradients, or decorative effects.
- Plain words, no jargon: "Start Broadcast", "Students connected: 42" — never "WebSocket" or "port".
- Light and dark mode, following the system setting.
- Consistent look across the add-in task pane, companion app, toolbar and student viewer, so it feels like one product.
- Accessible: WCAG AA contrast, large touch targets, full keyboard support, tooltips on every icon.
- Smooth, subtle animations only (fades, 150–200 ms). Nothing flashy.

### Add-in task pane
- Big "Start Broadcast" button as the main element.
- When live: a clear "Live" status, viewer count, and Pause / Blank / Stop buttons.
- Settings tucked away behind a small gear icon.

### QR window
- Large QR code, the short link in big readable text below it, and one line: "Scan to view the slides on your phone".
- Readable from the back of a classroom when shown on the projector.

### Floating toolbar
- Small pill-shaped bar with icon buttons: Pause/Resume, Blank, QR, Stop, plus viewer count.
- Draggable, collapsible, fades when idle and returns on hover; never covers slide content by default.
- Clear state: an obvious "Paused" or "Blanked" indicator so the teacher never forgets students are seeing a frozen or black screen.
- Stop asks for a quick confirmation to prevent accidental clicks.

### Student viewer
- The slide fills the screen. No buttons visible until tapped, then just Fullscreen.
- Pinch-to-zoom and landscape support.
- Friendly states: "Waiting for the presenter…", "Paused by your teacher", a black screen for Blank, "Reconnecting…" (with auto-reconnect), "The broadcast has ended".
- Loads fast on cheap Android phones and slow Wi-Fi.

### Setup wizard (first run, both platforms)
- 3–4 simple steps with a progress indicator and a green tick for each completed step.
- Checks each required permission (on macOS: Screen Recording and Local Network; on both: firewall/network access). For anything missing, one "Fix this" button that opens the right settings page.

### Polish
- Every error message says what happened and what to do next, in one short sentence.
- Before finishing, review every screen for clutter and remove anything that isn't essential.

---

## 5. Logo & branding

- The product is named **SlideRelay**.
- Design an original logo (simple, flat, readable at 16px) suggesting "presentation relayed to devices". Do not use or imitate Microsoft/PowerPoint branding.
- Deliver it as SVG plus all PNG sizes the Office add-in manifest and ribbon need (16, 32, 64, 80, 128 and any others required), Windows `.ico`, macOS `.icns`, Tauri app/tray icons, and a 256px version for installers and the README.

---

## 6. Repository, hosting, releases & updates

- **Repo:** https://github.com/mk-manishkandel/SlideRelay (public, MIT License). It already exists — work directly in it.
- **Budget: zero cost.** Distribute only via GitHub (Pages, Releases, Actions). No Microsoft AppSource, no paid services, no paid code signing.

### Add-in hosting & updates
- Host the Office.js add-in's web files on **GitHub Pages** (HTTPS), deployed automatically by GitHub Actions on every push to `main`. Add-in updates must reach users with no reinstall.
- Keep the manifest stable; document which changes require users to reinstall the manifest.
- Teachers install the add-in by sideloading the manifest; document this clearly for Windows and Mac.

### Companion app releases & auto-update
- GitHub Actions: pushing a tag like `v1.2.0` builds the Windows (MSI/NSIS) and macOS (universal DMG, Apple Silicon + Intel) installers and publishes a GitHub Release with release notes from `CHANGELOG.md`.
- Use the **Tauri updater plugin** with GitHub Releases as the update source (`latest.json`), signed with an updater key stored in GitHub Secrets. The app checks for updates on launch and offers one-click update.
- Builds must work unsigned. Prepare Windows builds for free **SignPath Foundation** signing (open-source program) and document how to enable it once approved. Ship macOS builds unsigned (no paid Apple Developer account); document optional future Apple signing + notarization.

### Repo maintenance
- Monorepo: `addin/` (Office.js), `companion/` (Tauri: Rust + TypeScript), `viewer/` (student page), `assets/` (logo/icons), `docs/`.
- CI on every push/PR: build + tests + lint (rustfmt, clippy, ESLint, Prettier).
- Semantic versioning; one version number shared by the add-in and the companion app.
- Files: README, CONTRIBUTING.md, CODE_OF_CONDUCT.md, SECURITY.md, CHANGELOG.md, PRIVACY.md, issue/PR templates.

---

## 7. Documentation

- **README:** what SlideRelay is, features, screenshot placeholders, quick start, links to the install guides, build guide for developers (Rust, Node, Tauri prerequisites), troubleshooting (firewall, macOS permissions, Wi-Fi client isolation, college networks), FAQ.
- **`docs/INSTALL-WINDOWS.md`:** step-by-step for non-technical teachers with screenshot placeholders, including the SmartScreen warning ("More info" → "Run anyway") until signing is in place, firewall prompt, and sideloading the add-in.
- **`docs/RELEASING.md`:** a beginner guide for making a change, pushing it, and publishing a new release (tagging, what GitHub Actions does, how users get the update).
- **`docs/APPSOURCE.md`:** short notes on what a future Microsoft AppSource submission would need (Partner Center account, privacy policy, disclosure of the companion app, testing notes). Not to be done now.

### `docs/INSTALL-MAC.md` (required, beginner-friendly)
Mac teachers will see security warnings because the app is unsigned. Link this guide prominently from the README. Write it as a numbered guide with a screenshot placeholder for every step. Explain upfront, in plain words, that the warnings are expected for free open-source apps and the app is safe. Cover:

1. **Before you start:** supported macOS versions (Apple Silicon and Intel); PowerPoint for Mac (Microsoft 365 or 2016+) installed; Mac on the same Wi-Fi as students.
2. **Download:** the latest `.dmg` from the GitHub Releases page (link it directly).
3. **Install:** open the `.dmg` and drag SlideRelay into the Applications folder.
4. **First launch (the "unverified developer" block):**
   - Double-click SlideRelay. macOS says it can't verify the developer. Click "Done"/"OK" (NOT "Move to Bin").
   - Open System Settings → Privacy & Security, scroll down to "Security", click "Open Anyway" next to the SlideRelay message.
   - Click "Open Anyway" again in the popup and enter the Mac password.
   - Explain this is needed only once.
5. **Allow Screen Recording:** System Settings → Privacy & Security → Screen & System Audio Recording → turn on SlideRelay → quit and reopen SlideRelay when macOS asks. Explain why (to capture only the slideshow window; nothing is recorded or saved).
6. **Allow Local Network:** click "Allow" when macOS asks if SlideRelay can find devices on the local network. Explain how to fix it later if they clicked "Don't Allow".
7. **Allow incoming connections:** if the macOS firewall asks, click "Allow" so students' phones can connect.
8. **Add SlideRelay to PowerPoint:** step-by-step manifest sideloading on Mac (where to place the manifest, restarting PowerPoint, finding SlideRelay under Home → Add-ins).
9. **Test it:** start a slideshow, click Start Broadcast, scan the QR with a phone.
10. **Troubleshooting:** app "damaged/can't be opened", "Open Anyway" button missing, black screen for students (Screen Recording off), phones can't connect (Wi-Fi client isolation, different networks), how to uninstall.

Verify these steps against current macOS versions and update the wording if macOS has changed them.

---

## 8. Quality

- Follow Microsoft's Office add-in guidelines (manifest, UI/design guidelines, icons, validation), Tauri security best practices (minimal permissions/capabilities), and platform guidelines for Windows and macOS.
- Must never slow down or crash PowerPoint; fail gracefully; log errors to a local log file.
- Keep CPU and memory usage low during broadcasting on both platforms.
- Write tests for the server and session logic (Rust unit/integration tests).
- Give me a manual test checklist for classroom scenarios on both Windows and Mac: single monitor, projector, Presenter View on/off, animations and video, 60 phones, Wi-Fi drop, slideshow restart, pause/blank/stop.

---

## 9. Skills to use (if available in your environment)

- **frontend-design** — for all UI: task pane, QR window, floating toolbar, setup wizard, student viewer and the logo.
- **pptx** — to generate test decks with animations, transitions, embedded video and many slides.
- **computer-use** — to operate PowerPoint on my machine for real end-to-end tests (slideshow, Presenter View, Broadcast).
- **chrome-browser** — to test the student viewer page (mobile layout, reconnection, console errors).
- **deep-research** — to check current Office.js, Tauri 2, Windows.Graphics.Capture and ScreenCaptureKit documentation before building.

If a skill isn't available, continue without it.

---

## 10. How to work

1. Before coding, ask me any questions where my answer would change the design.
2. Set up the repo structure, CI and GitHub Pages first, with a minimal working add-in ↔ companion connection.
3. Build capture and streaming next, and prove the "slideshow window only, never Presenter View" requirement early on both platforms.
4. Then the UI, setup wizard, release pipeline and documentation.
5. Work step by step, commit in small, clearly described commits, and tell me exactly how to install and test it on Windows and on Mac.
