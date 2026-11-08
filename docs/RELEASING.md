# Releasing SlideRelay

Releases are built by GitHub Actions from a version tag and published as a **draft**. A
maintainer checks the draft and clicks **Publish**. Only published releases reach users'
automatic updates.

The PowerPoint add-in is separate: every push to `main` deploys it to GitHub Pages
([pages.yml](../.github/workflows/pages.yml)), and the panel refreshes itself.

## One-time setup (maintainer)

Run this on your own computer, from the repository folder, signed in to the GitHub CLI:

```sh
bash scripts/setup-release-keys.sh
```

It creates, backs up in `~/.tauri/` and stores as GitHub Actions secrets:

| Secret                                                                      | What it's for                                                                                                                                                              |
| --------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`           | Signs updates. The app only installs updates signed with this key.                                                                                                         |
| `MACOS_CERTIFICATE`, `MACOS_CERTIFICATE_PASSWORD`, `MACOS_SIGNING_IDENTITY` | A self-made macOS signing certificate. It gives every Mac release the same identity, so Screen Recording permission survives updates. It is **not** an Apple Developer ID. |

It also writes the updater's **public** key into `companion/src-tauri/tauri.release.conf.json`.
Commit that change.

> **Back up `~/.tauri/sliderelay-*`** (for example in a password manager). If the updater key
> is lost, installed copies can't update automatically any more and users must download
> the new version by hand.

## Every release

1. **Pick the version** ([semver](https://semver.org)): bug fixes → patch (1.0.1), new features
   → minor (1.1.0), breaking changes → major.
2. **Set it everywhere:**

   ```sh
   npm run set-version -- 1.1.0
   ```

3. **Update [CHANGELOG.md](../CHANGELOG.md):** rename `[Unreleased]` to `[1.1.0] - YYYY-MM-DD`
   and start a new empty `[Unreleased]`. Write for presenters, in plain words.
4. **Commit, tag and push:**

   ```sh
   git commit -am "Release 1.1.0"
   git tag v1.1.0
   git push && git push --tags
   ```

5. **Wait for [Release](../.github/workflows/release.yml)** (about 20 minutes). It builds:
   - Windows: `SlideRelay_1.1.0_x64-setup.exe` (NSIS) and `SlideRelay_1.1.0_x64_en-US.msi`
   - Mac: `SlideRelay_1.1.0_universal.dmg`
   - Updater files (`*.sig`, `*.tar.gz`, `latest.json`) and `SHA256SUMS.txt`
6. **Check the draft** in [Releases](https://github.com/mk-manishkandel/SlideRelay/releases):
   install it on a Windows PC and a Mac, and run the [room test checklist](TEST-CHECKLIST.md).
7. Click **Edit → Publish release**. Installed copies pick up the update within a day.

If the tag doesn't match `package.json`, the workflow stops with a clear error. Fix it, delete
the tag (`git tag -d v1.1.0 && git push --delete origin v1.1.0`) and tag again.

## Test build without releasing

**Actions → Release → Run workflow** (leave _dry run_ ticked). The installers are attached to
the workflow run as artifacts; no release is created and nothing reaches users.

## Add-in manifest changes

`addin/manifest.xml` has its own version (`1.0.0.0`) and almost never changes: the add-in's
code updates through GitHub Pages. If you must change the manifest (name, icons, permissions),
bump its `<Version>`, and note in the changelog that presenters should click
**Add to PowerPoint** again in SlideRelay's setup guide. The app ships the manifest it was built
with.
