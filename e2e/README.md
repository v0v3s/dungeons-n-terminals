# End-to-end tests

dnt is tested the way it is used: the real programs, driven by keys and
taps, on every target it is released for.

| Target | What runs | Where |
|---|---|---|
| `tui` | the terminal app in a pseudo-terminal, read back through a terminal emulator | `crates/dnt/tests/e2e`, on Linux and Windows (CI) |
| `app` | the Tauri app through its WebView's DevTools protocol ([Playwright](https://playwright.dev)) | `app/e2e`, on an Android phone (USB) and on Windows (CI) |
| `android` | as `app`, phone only (touch) | `app/e2e` |

## The rule

**Every user-visible feature is a scenario in [`scenarios.toml`](scenarios.toml),
and every scenario has a test on every target it lists.** A new or changed
feature adds or updates its scenarios and their tests in the same change; a
bug fix adds the test that would have caught it. A frontend without the
feature says why (`not = { app = "…" }`).

`cargo test -p dnt --test scenarios` checks it: every scenario decides both
frontends, has a test per target, and every test names a scenario of the
catalog. It runs in CI with the other tests.

## What a test starts from

Each test starts from a known state, made through the same core the
programs use (never by hand-written files):

- **`empty`**: the first start: only the built-in SRD 5.2.1;
- **`data`**: and the test supplement imported ([`data/`](data) and
  [`homebrew/`](homebrew));
- **`campaign`**: and the campaign *E2E*, active;
- **`party`**: and the test party, made through character creation as
  [`characters.json`](characters.json) describes (a Wizard, a Monk, a
  Warlock and a Ranger at level 1, from the SRD).

In the terminal: `Home::empty()`, `Home::data()`, `Home::campaign()`,
`Home::party()`. In the app: `test.use({ seed: "data" })` (the party by
default); the app is reset before every test.

## The test data

The tests use the **SRD 5.2.1**, built into dnt ([`data/srd`](../data/srd),
made from the SRD's PDF by [`scripts/srd`](../scripts/srd)): its classes,
species, backgrounds, feats, spells, items and rules.

[`data/`](data) is a small supplement of **invented** content for what the
SRD lacks: a second book (*The Proving Grounds*, `TST`) with a class whose
subclass comes at level 1 (the Hexer) and a species with subraces (the
Stoneling), an older book (`TSX`) whose Fire Bolt the SRD reprints, a
disease, a status and an adventure. [`homebrew/`](homebrew) is a homebrew
collection (`TSTB`).

A feature that reads something the SRD does not have adds it to the
supplement, invented, and keeps it valid:

```sh
cargo run --release -p dnt-data --example validate -- e2e/data
```

## Running them

```sh
cargo test -p dnt --test e2e               # the terminal app (any system)
cargo test -p dnt --test e2e inventory     # some of them
cargo test -p dnt --test scenarios         # the rule

cd app
npm run e2e:android                        # a test phone over USB: builds, installs, runs
npm run e2e:android -- --no-build -g notes # some of them, without building
npm run e2e:windows                        # on Windows
npx playwright show-report e2e/report      # what failed, with screenshots
```

The app's tests **reset the app**: on a phone, use a test device (they
refuse to run when the app has campaigns of its own; `DNT_E2E_RESET=1` wipes
them anyway). On Windows the app runs in a temporary home (`DNT_HOME`). The
terminal tests each run in a temporary `DNT_HOME` too, with repeatable dice
(`DNT_SEED`).

## Writing one

Terminal:

```rust
#[test]
fn unpacking_a_pack() {
    let mut t = Tui::scenario("inventory.unpack", Home::party());
    t.send("2").wait("Tester");
    t.send(ENTER).wait("Overview");
    // …keys, then what the user sees:
    t.wait("Unpacked Scribe's Pack");
}
```

App:

```ts
test("a pack unpacks into its container", { tag: "@inventory.unpack" }, async ({ page }) => {
  await openSheet(page, "Inventory");
  // …taps, then what the user sees:
  await expect(status(page)).toContainText("Unpacked Scribe's Pack");
});
```

Test what the user sees, not the code: keys and taps in, the screen out.
