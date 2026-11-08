# Deploying the add-in for an organisation

Most presenters don't need this page: SlideRelay's setup guide adds the **Broadcast** button
to PowerPoint by itself. This page is for **Microsoft 365 administrators** who want the button
on every PowerPoint in their organisation, and for maintainers considering Microsoft AppSource.

## Centralised deployment (Microsoft 365 admin center)

Admins can push the add-in to users or groups. It then appears in PowerPoint for Windows, Mac
and the web without any per-computer steps.

1. Download the manifest: https://mk-manishkandel.github.io/SlideRelay/manifest.xml
2. Go to the [Microsoft 365 admin center](https://admin.microsoft.com) → **Settings →
   Integrated apps → Upload custom apps**.
3. Choose **Office Add-in**, then **Upload manifest file (.xml) from device**, and pick
   `manifest.xml`.
4. Choose who gets it (everyone, specific users or groups) and **Deploy**.

It can take up to 24 hours to show up. Users still need the **SlideRelay app** on their computer
(from [Releases](https://github.com/mk-manishkandel/SlideRelay/releases/latest)); IT can deploy
the `.msi` with Intune, Configuration Manager or Group Policy:

```powershell
msiexec /i SlideRelay_x.y.z_x64_en-US.msi /qn
```

The MSI doesn't add a Windows Firewall rule. Add one centrally (inbound, program
`C:\Program Files\SlideRelay\SlideRelay.exe`, allow), or users will be asked on first start.

### What the add-in needs

- **Permissions:** `Restricted` (the lowest level). It can't read or change documents.
- **Network:** loads from `https://mk-manishkandel.github.io/SlideRelay/` and Microsoft's
  Office.js CDN. It talks to the SlideRelay app on `127.0.0.1:47615` / `localhost:47616`
  (this computer only).
- **Data:** none collected. See [PRIVACY.md](../PRIVACY.md).

## Microsoft AppSource (future)

Listing on AppSource would let anyone install the add-in from **Home → Add-ins** inside
PowerPoint. Checklist before submitting through
[Partner Center](https://partner.microsoft.com/dashboard/office/products):

- [ ] Partner Center developer account (free for individuals).
- [ ] Privacy policy URL ([PRIVACY.md](../PRIVACY.md)) and support URL (GitHub Issues). ✓ exist
- [ ] Manifest `SupportUrl`, icons (32, 64, 128 px) and `Restricted` permissions. ✓
- [ ] Test notes for Microsoft's reviewers: the add-in needs the companion app, so give them
      the release link and these steps.
- [ ] Screenshots of the panel (1366×768).
- [ ] Accept that AppSource validation may require the add-in to show useful content without
      the app (it shows install instructions: ✓).
