# Install SlideRelay on Windows

About 5 minutes. You need Windows 10 or 11 (64-bit) and the PowerPoint desktop app.

## 1. Download

Click **[Download for Windows](https://github.com/mk-manishkandel/SlideRelay/releases/latest/download/SlideRelay-Windows-setup.exe)**.
It always gives you the newest version (`SlideRelay-Windows-setup.exe`).

> Only download SlideRelay from **github.com/mk-manishkandel/SlideRelay/releases**.
> To check the file, see [Check your download](#check-your-download-optional) below.

The [MSI package](https://github.com/mk-manishkandel/SlideRelay/releases/latest/download/SlideRelay-Windows.msi)
is for IT departments who deploy software to many PCs. Most people want the `setup.exe`.

### "This file isn't commonly downloaded" / "could harm your device"

Your browser may warn you while downloading. **This is not a virus warning.** It appears because
SlideRelay is new, free and not yet signed with a paid certificate, so few people have downloaded
this exact file yet (see [SIGNING.md](SIGNING.md)). To keep the file:

- **Microsoft Edge:** in the downloads panel, hover over the file → **⋯** → **Keep** →
  **Show more** → **Keep anyway**.
- **Google Chrome:** click **Keep** (or **⋮** → **Download dangerous file**).

Want to be sure? [Check your download](#check-your-download-optional): if the checksum matches,
it's exactly the file GitHub built from the public source code.

If **Microsoft Defender** ever reports a threat in SlideRelay, that's a false positive: please
[tell us](https://github.com/mk-manishkandel/SlideRelay/issues/new/choose) and report it to
Microsoft at https://www.microsoft.com/wdsi/filesubmission (choose **Incorrect detection**).

## 2. Install

1. Double-click the downloaded `SlideRelay-Windows-setup.exe`.
2. **"Windows protected your PC"** may appear (Microsoft Defender SmartScreen). It appears
   because SlideRelay isn't signed with a paid certificate yet, and new downloads are rare
   (see [SIGNING.md](SIGNING.md)). Click **More info**, check that it says
   **App: SlideRelay-Windows-setup.exe**, then click **Run anyway**.
3. Choose who to install for:
   - **Anyone who uses this computer (all users)** _(recommended if you can)_: Windows asks for
     an administrator. The installer then also lets viewers through Windows Firewall, so
     there's nothing else to set up.
   - **Only for me**: no administrator needed. Windows will ask about the firewall the first
     time SlideRelay starts (step 3).
4. Click **Install**, then **Finish**. SlideRelay starts.

## 3. Allow viewers through the firewall (only if Windows asks)

If you installed **only for me**, Windows Defender Firewall may show
**"Windows Defender Firewall has blocked some features of this app"** for SlideRelay.

- Tick **Private networks**.
- Also tick **Public networks** if your room's Wi-Fi is marked "Public" on this PC (common on
  school and office Wi-Fi). To check: **Settings → Network & internet → Wi-Fi → (your network)**.
- Click **Allow access**. If it asks for an administrator and you don't have one, ask your IT
  team to allow **SlideRelay** in Windows Firewall.

Without this, viewers' phones can't reach SlideRelay: the link times out.

## 4. Run the setup guide

The first time SlideRelay starts, its setup guide opens:

1. Click **Add to PowerPoint**. It shows a green check when done.
2. Click **Done**. SlideRelay hides in the **system tray** (next to the clock; you may need to
   click the **^** arrow). Right-click the icon for **Setup and status** or **Quit**.
3. **Quit and reopen PowerPoint.**

## 5. Your first broadcast

1. In PowerPoint, open your presentation and click **Home → Broadcast** (far right of the Home
   tab).
2. The first time only: click **Connect**, then **Allow** in the SlideRelay window that
   appears. This lets the PowerPoint panel control SlideRelay.
3. Click **Start Broadcast**. The panel shows a QR code and a link.
4. Viewers scan the QR code (or type the link) on the **same Wi-Fi** as you.
5. Start your slideshow (**F5**). Viewers see it live.

While presenting, use the floating toolbar or these shortcuts:

|                                                             |                |
| ----------------------------------------------------------- | -------------- |
| Pause / resume (viewers keep the current slide)             | **Ctrl+Alt+P** |
| Blank / show (viewers see "Screen hidden by the presenter") | **Ctrl+Alt+B** |
| Show / hide the QR code on the projector                    | **Ctrl+Alt+Q** |

When you click **Stop Broadcast**, viewers see that it ended and SlideRelay closes. Turn this
off in the panel's settings (gear icon) if you prefer it to keep running.

Before your first real presentation, try the [room test checklist](TEST-CHECKLIST.md).

## Updates

SlideRelay checks for updates in the background (only when online). An update downloads
quietly and installs when you stop a broadcast or quit, never during a presentation. The
PowerPoint panel updates by itself.

## Check your download (optional)

In the release's **Assets**, open `SHA256SUMS.txt`. Then in PowerShell:

```powershell
Get-FileHash "$env:USERPROFILE\Downloads\SlideRelay-Windows-setup.exe"
```

The **Hash** must match the line for that file (letter case doesn't matter).

## Troubleshooting

**No Broadcast button in PowerPoint.** Quit PowerPoint completely (check the taskbar) and open it
again. Still missing? Open SlideRelay from the tray (**Setup and status**) and click
**Add to PowerPoint** again. In PowerPoint, also try **Home → Add-ins → More add-ins →
Developer add-ins / My add-ins → SlideRelay**.

**The panel says "The SlideRelay app is not running".** Click **Open SlideRelay** in the panel,
or start SlideRelay from the Start menu.

**Viewers' phones can't open the link.**

- They must be on the **same Wi-Fi** as your PC (not mobile data, not a guest network).
- Check the firewall (step 3).
- Some networks block devices from talking to each other ("client isolation"), often guest Wi-Fi.
  Ask IT, or use a phone hotspot or a small travel router.
- On a PC with several networks (VPN, Ethernet and Wi-Fi), choose the right one in the panel:
  **gear icon → Network for the viewer link**.

**Viewers see "Waiting for the presenter…".** Start the slideshow. If Presenter View is on and you
have only one screen, SlideRelay won't show the presenter screen (it contains your notes): turn
off **Slide Show → Use Presenter View**, or connect a projector.

**The picture is blurry or slow.** Panel → gear icon → **Picture quality**: _Low_ for weak Wi-Fi,
_High_ for small text.

**Logs for a bug report:** `%LOCALAPPDATA%\io.github.mkmanishkandel.sliderelay\logs\`. Please
attach the latest file to a [bug report](https://github.com/mk-manishkandel/SlideRelay/issues/new/choose).

## Uninstall

**Settings → Apps → Installed apps → SlideRelay → Uninstall.** This also removes the firewall rule.
To remove the PowerPoint button too, delete the value `7fb1353c-bb37-4bf0-8739-9517d4ca3377`
under `HKEY_CURRENT_USER\Software\Microsoft\Office\16.0\WEF\Developer` (Registry Editor), and
the folder `%APPDATA%\io.github.mkmanishkandel.sliderelay`.
