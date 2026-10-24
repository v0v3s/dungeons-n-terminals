#!/usr/bin/env sh
# Run the end-to-end tests on an Android device (USB debugging on).
#
#   scripts/e2e-android.sh (--zip DATA.zip | --data DIR) [--no-build] [playwright args…]
#
#   --zip FILE   the data zip to import (made by scripts/pack-data.sh)
#   --data DIR   or a data repository, packed into one first
#   --no-build   install the last e2e build as it is
#
# The tests RESET the app on the device: use a test device. They refuse to
# run when the app has campaigns other than theirs (DNT_E2E_RESET=1 wipes
# them anyway). With more than one device, DNT_DEVICE picks one.
#
# The build has the e2e feature (the WebView can be debugged), so it is
# never one to release. Signed with the release key when there is one, it
# installs over the release app; otherwise uninstall that first.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
zip=""
data=""
build=1
while [ $# -gt 0 ]; do
  case "$1" in
    --zip) zip=$2; shift 2 ;;
    --data) data=$2; shift 2 ;;
    --no-build) build=0; shift ;;
    -h | --help) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) break ;;
  esac
done

if [ -n "$data" ]; then
  zip=$(mktemp -d)/e2e-data.zip
  sh "$here/../scripts/pack-data.sh" --data "$data" -o "$zip"
fi
[ -n "$zip" ] && [ -f "$zip" ] || { echo "a data zip is needed: --zip FILE or --data DIR (see --help)" >&2; exit 2; }

serial=${DNT_DEVICE:-$(adb devices | awk 'NR > 1 && $2 == "device" { print $1; exit }')}
[ -n "$serial" ] || { echo "no device: connect one with USB debugging on (adb devices)" >&2; exit 2; }
export DNT_DEVICE="$serial"

export DNT_E2E_ZIP=$(cd "$(dirname "$zip")" && pwd)/$(basename "$zip")

cd "$here"
if [ "$build" = 1 ]; then
  (. scripts/android-env.sh && npx tauri android build --apk --target aarch64 --features e2e)
fi
adb -s "$serial" install -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
adb -s "$serial" shell am force-stop cz.v0v3s.dnt
npx playwright test "$@"
