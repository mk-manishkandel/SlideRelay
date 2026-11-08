# Code signing

Code signing tells Windows and macOS who made an app. SlideRelay is free and open source with
zero running costs, so it uses free options where they exist.

|                       | Today                                                                                                                                                                  | Plan                                                                                                           |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| **Updates**           | Signed with the project's updater key ([RELEASING.md](RELEASING.md)). The app refuses unsigned, tampered or older updates.                                             | Done                                                                                                           |
| **Windows installer** | Not signed. SmartScreen shows "Windows protected your PC".                                                                                                             | Apply to the [SignPath Foundation](https://signpath.org) (free code signing for open-source projects).         |
| **Mac app**           | Signed with the project's own certificate (stable identity, so permissions survive updates), **not** notarized by Apple. macOS asks the user to confirm on first open. | Apple notarization needs a paid Apple Developer account (99 USD a year). Add it if the project gets a sponsor. |

## Windows: SignPath Foundation

SignPath signs releases built by GitHub Actions, so the signed file provably comes from this
repository's source.

1. Check the [conditions](https://signpath.org/terms) (OSI license ✓, no proprietary parts ✓,
   builds on a public CI ✓, project already released ✓).
2. Apply at https://signpath.org/apply, naming this repository and the release workflow.
3. After approval, SignPath gives you an organisation ID, a project slug and a signing policy.
   Add these GitHub secrets: `SIGNPATH_API_TOKEN`, `SIGNPATH_ORGANIZATION_ID`.
4. In [release.yml](../.github/workflows/release.yml), at the marked spot in the Windows build,
   upload the unsigned `.exe` and `.msi` as an artifact, then add
   [`signpath/github-action-submit-signing-request`](https://github.com/SignPath/github-action-submit-signing-request)
   with `wait-for-completion: true`, and upload the signed files to the release instead.
5. Mention the signer on the release page: _"Free code signing provided by SignPath.io,
   certificate by SignPath Foundation"_ (SignPath requires this).

Windows reputation (SmartScreen) builds up over downloads, even for signed files, so the
warning may still show for a while after the first signed release.

## macOS: notarization (if funded)

With an Apple Developer account, set these secrets instead of the self-made certificate:
`APPLE_CERTIFICATE` (Developer ID Application, .p12, base64), `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific password) and
`APPLE_TEAM_ID`. Tauri then signs **and** notarizes, and the first-open warning disappears.
Note: changing the signing identity resets users' Screen Recording permission once.
