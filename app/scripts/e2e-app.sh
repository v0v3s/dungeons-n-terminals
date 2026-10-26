#!/usr/bin/env sh
# Run the app's end-to-end tests (app/e2e) on a target.
#
#   scripts/e2e-app.sh [--target android|windows] [--no-build] [playwright args…]
#
#   --target   android (default): a phone over adb (USB debugging on);
#              windows: the desktop app, on Windows
#   --no-build use the last e2e build as it is
#
# The tests use the repository's test data (e2e/data, e2e/homebrew) and
# party (e2e/characters.json), and RESET the app before each test. On a
# phone, use a test device: they refuse to run when the app has campaigns
# of its own (DNT_E2E_RESET=1 wipes them anyway); DNT_DEVICE picks one of
# several devices. On Windows the app runs in a temporary home (DNT_HOME).
#
# The build has the e2e feature (its WebView can be debugged), so it is
# never one to release.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
target=android
build=1
while [ $# -gt 0 ]; do
  case "$1" in
    --target) target=$2; shift 2 ;;
    --no-build) build=0; shift ;;
    -h | --help) sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) break ;;
  esac
done

zip=$(mktemp -d)/e2e-data.zip
sh "$here/../scripts/pack-data.sh" --data "$here/../e2e/data" --homebrew "$here/../e2e/homebrew" -o "$zip" >/dev/null
# Node on Windows needs Windows paths (Git Bash gives /tmp/…).
native() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else echo "$1"; fi; }
export DNT_E2E_ZIP="$(native "$zip")" DNT_E2E_TARGET="$target"

cd "$here"
case "$target" in
  android)
    serial=${DNT_DEVICE:-$(adb devices | awk 'NR > 1 && $2 == "device" { print $1; exit }')}
    [ -n "$serial" ] || { echo "no device: connect one with USB debugging on (adb devices)" >&2; exit 2; }
    export DNT_DEVICE="$serial"
    if [ "$build" = 1 ]; then
      (. scripts/android-env.sh && npx tauri android build --apk --target aarch64 --features e2e)
    fi
    adb -s "$serial" install -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
    adb -s "$serial" shell am force-stop cz.v0v3s.dnt
    ;;
  windows)
    if [ "$build" = 1 ]; then
      # WebView2 opens the DevTools port through the window's browser
      # arguments (the app's own replace WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS):
      # the app's windows, with the port added to the default arguments.
      conf=$(mktemp -d)/e2e.conf.json
      node --input-type=module -e '
        import { readFileSync, writeFileSync } from "node:fs";
        const c = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
        const args = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port=9333";
        const windows = c.app.windows.map((w) => ({ ...w, additionalBrowserArgs: args }));
        writeFileSync(process.argv[1], JSON.stringify({ app: { windows } }));
      ' "$(native "$conf")"
      npx tauri build --no-bundle --features e2e --config "$(native "$conf")"
    fi
    export DNT_E2E_APP="$(native "$here/../target/release/dnt-app.exe")"
    echo "the app: $DNT_E2E_APP"; ls -la "$here/../target/release/" | grep -i dnt || true
    set -- --grep-invert @android "$@"
    ;;
  *) echo "unknown target: $target (android or windows)" >&2; exit 2 ;;
esac
npx playwright test "$@"
