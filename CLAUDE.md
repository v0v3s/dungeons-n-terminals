# dnt — working in this repository

## THE RULE: every feature has end-to-end scenarios. Always. No exceptions.

Any change that adds or changes something a user can do or see — in the
terminal app (`crates/dnt`), the app (`app/`), or the shared core they
both call — ships in the same change with:

1. **Its scenarios in `e2e/scenarios.toml`**: a new `[[scenario]]` for a new
   feature, the existing one updated for a changed feature. Each scenario
   decides both frontends: a target (`"tui"`, `"app"`, `"android"`), or
   a written reason the frontend does not have it (`not = { … }`).
2. **A test for every target it lists**, from a user's point of view:
   - terminal: `crates/dnt/tests/e2e/*.rs`, a test that starts with
     `Tui::scenario("<id>", Home::…)` and drives the real binary with keys;
   - app: `app/e2e/*.spec.ts`, a test tagged `{ tag: "@<id>" }` that taps
     and types like a user (the same specs run on Android and Windows).
3. **The test data it needs**, in `e2e/data` (invented content only,
   validated by `cargo run -p dnt-data --example validate -- e2e/data`) or
   the party in `e2e/characters.json`, when the feature reads something the
   data does not have yet.

A feature is not done until its scenario tests pass on every target. Fixing
a bug adds the test that would have caught it. Removing a feature removes
its scenarios. `cargo test -p dnt --test scenarios` fails on any gap, and
CI runs it on every push: never weaken it, skip a test, or mark a
scenario's frontend as `not` to get past it.

Running them: `e2e/README.md`.

## Also

- Never mention the source site of the game data, anywhere (code, docs,
  commits). The data format is "the data format"; test data is invented.
- Commits follow Conventional Commits; the PR title sets the release
  version. Changes go under `## Unreleased` in `CHANGELOG.md`.
- `cargo fmt --all` and `cargo clippy --workspace --all-targets` before
  pushing; `npm run check` in `app/` for the UI.
- No personal data in the repository: it is public.
