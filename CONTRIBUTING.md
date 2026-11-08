# Contributing to SlideRelay

Thanks for helping! Bug reports, ideas, docs fixes and code are all welcome.

## Reporting bugs and ideas

Use [Issues](https://github.com/mk-manishkandel/SlideRelay/issues/new/choose). For bugs,
please include your system (Windows/Mac version, PowerPoint version), what you did, what you
expected, and the log file (see the install guides). Security problems: see
[SECURITY.md](SECURITY.md) instead.

## Code

1. Set up your computer: [docs/DEVELOPING.md](docs/DEVELOPING.md).
2. Create a branch, make your change, and add or update tests where it makes sense.
3. Run the same checks as CI:

   ```sh
   npm run lint && npm run typecheck && npm run build && npm test
   cd companion/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
   ```

4. Open a pull request describing **what changed for presenters or viewers**.

## Style

- **Plain words** in anything people read: short sentences, no jargon. Say _presenter_ and
  _viewer_.
- **One accent colour** (`#2563EB`), light and dark themes, WCAG AA contrast. Use the tokens
  in `shared/theme.css`.
- **Privacy first:** never capture anything but the slideshow window, and never add telemetry.
- Keep the add-in ↔ app protocol in `shared/protocol.ts` and `control_api.rs` in sync, and
  follow its versioning rules.

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE), and to follow our [Code of Conduct](CODE_OF_CONDUCT.md).
