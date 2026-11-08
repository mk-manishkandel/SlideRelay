# Install SlideRelay on a Mac

About 5 minutes. You need macOS 13 Ventura or later (Apple silicon or Intel) and the PowerPoint
desktop app.

## 1. Download

Click **[Download for Mac](https://github.com/mk-manishkandel/SlideRelay/releases/latest/download/SlideRelay-Mac.dmg)**.
It always gives you the newest version (`SlideRelay-Mac.dmg`), and one download works on every
Mac (Apple silicon and Intel).

> Only download SlideRelay from **github.com/mk-manishkandel/SlideRelay/releases**.
> To check the file, see [Check your download](#check-your-download-optional) below.

## 2. Install

1. Double-click the downloaded `.dmg`.
2. Drag **SlideRelay** onto the **Applications** folder.
3. Eject the disk image (the ⏏ button next to it in Finder).

## 3. Open it the first time

SlideRelay isn't notarized by Apple yet: that needs a paid Apple Developer account (see
[SIGNING.md](SIGNING.md)). So the first time, macOS blocks it. You only do this once.

1. Open **Applications** and double-click **SlideRelay**.
2. macOS says **"SlideRelay" Not Opened** (or "Apple could not verify…"). Click **Done** (or
   **OK**). Don't click Move to Trash.
3. Open **System Settings → Privacy & Security**. Scroll down to **Security**. Next to
   _"SlideRelay" was blocked to protect your Mac_, click **Open Anyway**.
4. Enter your Mac password (or Touch ID), then click **Open Anyway** again.

> **macOS 13 or 14:** you can instead right-click (or Control-click) SlideRelay in
> Applications, choose **Open**, then click **Open** in the message.

## 4. Run the setup guide

SlideRelay's setup guide opens. Each step shows a green check when done.

1. **Add SlideRelay to PowerPoint**: click **Add to PowerPoint**. If macOS asks
   **"SlideRelay" would like to access data from other apps**, click **Allow**: SlideRelay puts
   one small file in PowerPoint's add-in folder, nothing else.
2. **Let PowerPoint connect**: click the button and enter your Mac password. This trusts a
   certificate that only works on this Mac (named _SlideRelay local connection_), so the
   PowerPoint panel can talk to SlideRelay securely.
3. **Allow Screen Recording**: click **Open System Settings**, switch **SlideRelay** on, and
   click **Quit & Reopen** if macOS asks. SlideRelay only uses this to see the slideshow window.
4. Click **Done**. SlideRelay hides in the **menu bar** (top right). Click its icon for
   **Setup and status** or **Quit SlideRelay**.
5. **Quit PowerPoint (⌘Q) and open it again.**

## 5. Your first broadcast

1. In PowerPoint, open your presentation and click **Home → Broadcast** (far right of the Home
   tab).
2. The first time only: click **Connect**, then **Allow** in the SlideRelay window that
   appears. This lets the PowerPoint panel control SlideRelay.
3. Click **Start Broadcast**. The panel shows a QR code and a link.
4. Viewers scan the QR code (or type the link) on the **same Wi-Fi** as you.
5. Start your slideshow (**⌘↩** or **Slide Show → From Beginning**). Viewers see it live.

While presenting, use the floating toolbar or these shortcuts:

|                                                             |                            |
| ----------------------------------------------------------- | -------------------------- |
| Pause / resume (viewers keep the current slide)             | **⌃⌥P** (Control-Option-P) |
| Blank / show (viewers see "Screen hidden by the presenter") | **⌃⌥B**                    |
| Show / hide the QR code on the projector                    | **⌃⌥Q**                    |

When you click **Stop Broadcast**, viewers see that it ended and SlideRelay quits. Turn this
off in the panel's settings (gear icon) if you prefer it to keep running.

Before your first real presentation, try the [room test checklist](TEST-CHECKLIST.md).

## Updates

SlideRelay checks for updates in the background (only when online). An update downloads
quietly and installs when you stop a broadcast or quit, never during a presentation. The
PowerPoint panel updates by itself. Screen Recording permission is kept across updates.

## Check your download (optional)

In the release's **Assets**, open `SHA256SUMS.txt`. Then in **Terminal**:

```sh
shasum -a 256 ~/Downloads/SlideRelay-Mac.dmg
```

The long code must match the line for that file.

## Troubleshooting

**No Broadcast button in PowerPoint.** Quit PowerPoint with **⌘Q** and open it again. Still
missing? Click the SlideRelay menu-bar icon → **Setup and status** → **Add to PowerPoint**.
In PowerPoint, also look under **Home → Add-ins**.

**The panel says "The SlideRelay app is not running".** Click **Open SlideRelay** in the panel.
If SlideRelay is already running, open **Setup and status** from its menu-bar icon and check
that **Let PowerPoint connect** has a green check.

**"SlideRelay needs Screen Recording permission" even though it's switched on.** In
**System Settings → Privacy & Security → Screen & System Audio Recording**, select SlideRelay,
remove it with **−**, then click **Fix this** in the panel and switch it on again. Quit and
reopen SlideRelay.

**Viewers' phones can't open the link.**

- They must be on the **same Wi-Fi** as your Mac (not mobile data, not a guest network).
- Some networks block devices from talking to each other ("client isolation"), often guest Wi-Fi.
  Ask IT, or use a phone hotspot or a small travel router.
- With several networks (VPN, Ethernet and Wi-Fi), choose the right one in the panel:
  **gear icon → Network for the viewer link**.

**Viewers see "Waiting for the presenter…".** Start the slideshow. If Presenter View is on and you
have only one screen, SlideRelay won't show the presenter screen (it contains your notes): turn
off **Slide Show → Presenter View**, or connect a projector.

**The panel looks out of date.** Quit PowerPoint, then delete the folder
`~/Library/Containers/com.microsoft.Powerpoint/Data/Library/Caches` and open PowerPoint again.

**Logs for a bug report:** `~/Library/Logs/io.github.mkmanishkandel.sliderelay/`. Please attach
the latest file to a [bug report](https://github.com/mk-manishkandel/SlideRelay/issues/new/choose).

## Uninstall

1. Click the menu-bar icon → **Quit SlideRelay**, then drag **SlideRelay** from Applications to
   the Bin.
2. Optional clean-up:
   - Remove the PowerPoint button: delete `SlideRelay.xml` from
     `~/Library/Containers/com.microsoft.Powerpoint/Data/Documents/wef/`.
   - Remove the local certificate: open **Keychain Access**, search for
     **SlideRelay local connection** and delete it.
   - Remove settings: delete `~/Library/Application Support/io.github.mkmanishkandel.sliderelay/`.
   - Remove the Screen Recording entry in **System Settings → Privacy & Security**.
