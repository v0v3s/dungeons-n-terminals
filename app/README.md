# dnt app (Windows and Android)

The graphical dnt, built with [Tauri 2](https://v2.tauri.app): the Rust side
(`src-tauri/`) calls `dnt-core`, the web UI (`src/`, Svelte 5 + TypeScript)
draws it in the terminal look of the TUI (see `src/app.css`).

```sh
cd app
npm install
npm run tauri dev            # a desktop window, with hot reload
npm run tauri android dev    # an emulator or a phone over adb (see below)
npm run check                # type-check the UI
```

Android needs JDK 17 and the Android SDK with an NDK; `source
scripts/android-env.sh` sets `JAVA_HOME`, `ANDROID_HOME` and `NDK_HOME` for
an SDK in `~/Android/Sdk`. `src-tauri/gen/android/` is the generated Android
project (kept in git, so changes to it stay).

The Rust crate (`dnt-app`) is a workspace member but not a default one:
`cargo clippy -p dnt-app` needs the UI built first (`npm run build`).

Data comes in as a zip (Data tab): a whole data folder, or homebrew files.
`scripts/pack-data.sh --data DIR --homebrew DIR` packs just what the app
reads into one small zip (see the main README). Homebrew stays off until
a campaign enables it (Sources › Homebrew).
On a desktop it is stored in the app's data folder
(`%APPDATA%\cz.v0v3s.dnt` on Windows), on Android in the app's storage.

## Tests on a device

`e2e/` holds end-to-end tests ([Playwright](https://playwright.dev)) that
drive the app on an Android phone over USB: they reach the app's WebView
over the DevTools protocol and press its buttons, as a user would.

```sh
npm run e2e:android -- --data ~/path/to/data    # or --zip dnt-data.zip
npm run e2e:android -- --no-build --zip dnt-data.zip -g notes   # some of them
npx playwright show-report e2e/report           # what failed, with screenshots
```

The script builds the app with the `e2e` feature (the WebView can be
debugged; never a build to release), installs it and runs the tests. They
**reset the app** and start from a known state: the data imported, a
campaign *E2E* and a level 1 wizard *Tester*. So use a test device: they
refuse to run when the app has other campaigns (`DNT_E2E_RESET=1` wipes
them anyway). With more devices connected, `DNT_DEVICE` picks one.

## Releases

Each release builds the Windows installer (`dnt-app-windows-x86_64-setup.exe`)
and, when the signing key is set up, the Android app (`dnt-app-android.apk`,
for phones, tablets and the emulator).

Android only installs updates signed with the same key, so create one once
and keep it safe (losing it means users must uninstall to update):

```sh
keytool -genkeypair -keystore dnt.jks -alias dnt -keyalg RSA -keysize 4096 -validity 36500
base64 -w0 dnt.jks   # the value of the ANDROID_KEYSTORE secret
```

Then add the repository secrets `ANDROID_KEYSTORE`, `ANDROID_KEY_ALIAS`
(`dnt`) and `ANDROID_KEY_PASSWORD`. For a signed build on your machine, put
a `keystore.properties` in `src-tauri/gen/android/` (ignored by git):

```
storeFile=/path/to/dnt.jks
keyAlias=dnt
password=…
```

