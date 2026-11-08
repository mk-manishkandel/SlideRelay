# Developing SlideRelay

## Prerequisites (install once)

| Tool                 | Mac                                   | Windows                                                                                             |
| -------------------- | ------------------------------------- | --------------------------------------------------------------------------------------------------- |
| Node.js 20+          | `brew install node` or nodejs.org     | nodejs.org installer                                                                                |
| Rust (stable)        | https://rustup.rs                     | https://rustup.rs (choose the MSVC toolchain)                                                       |
| Platform build tools | `xcode-select --install`              | "Visual Studio Build Tools" with **Desktop development with C++**; WebView2 (built into Windows 11) |
| rustfmt + clippy     | `rustup component add rustfmt clippy` | same                                                                                                |
| librsvg (icons only) | `brew install librsvg`                | not needed                                                                                          |

More detail: https://v2.tauri.app/start/prerequisites/

## First-time setup

```sh
git clone https://github.com/mk-manishkandel/SlideRelay.git
cd SlideRelay
npm install
```

## Run the companion app

```sh
npm run dev:companion
```

This builds the Rust app and opens the SlideRelay window. It also starts the localhost API
on `127.0.0.1:47615`. Quick check from a terminal:

```sh
curl http://127.0.0.1:47615/v1/hello
```

Logs are written to:

- Mac: `~/Library/Logs/io.github.mkmanishkandel.sliderelay/`
- Windows: `%LOCALAPPDATA%\io.github.mkmanishkandel.sliderelay\logs\`

## Run the add-in locally

Office only loads add-ins over HTTPS, so create a trusted local certificate once:

```sh
npx office-addin-dev-certs install
npm run dev:addin
```

The dev server runs on `https://localhost:3000` and writes `addin/manifest.dev.xml`.
Sideload **that** file (it is called "SlideRelay (dev)" so it can sit next to the real one).

Usually you don't need the dev server at all: every push to `main` deploys the add-in to
GitHub Pages within a minute or two, and the production manifest picks it up.

## Checks (same as CI)

```sh
npm run lint        # ESLint + Prettier
npm run typecheck
npm run build
cd companion/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

`npm run format` fixes formatting for you; `cargo fmt` does the same for Rust.

## How the add-in talks to the app

- The add-in page (from GitHub Pages, HTTPS) calls `http://127.0.0.1:47615`. Browsers allow
  this because loopback counts as a secure address. The app answers Chromium's
  "Private Network Access" preflight.
- Security: the app only listens on 127.0.0.1, checks the `Host` header (blocks DNS rebinding),
  only accepts the add-in's web origin, and requires a token. The presenter approves the token once
  by clicking **Allow** in the app ("pairing"). Only a hash of the token is saved on disk.
- The API is versioned: see `shared/protocol.ts` (TypeScript) and
  `companion/src-tauri/src/control_api.rs` (Rust). Keep them in sync.
