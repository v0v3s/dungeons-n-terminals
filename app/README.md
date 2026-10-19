# dnt app (Windows and Android)

The graphical dnt, built with [Tauri 2](https://v2.tauri.app): the Rust side
(`src-tauri/`) calls `dnt-core`, the web UI (`src/`, Svelte 5 + TypeScript)
draws it in the terminal look of the TUI (see `src/app.css`).

```sh
cd app
npm install
npm run tauri dev            # a desktop window, with hot reload
npm run tauri android dev    # an emulator or a phone over adb (needs JDK 17, Android SDK + NDK)
npm run check                # type-check the UI
```

The Rust crate (`dnt-app`) is a workspace member but not a default one:
`cargo clippy -p dnt-app` needs the UI built first (`npm run build`).

Data comes in as a zip (Data tab): a whole data folder, or homebrew files.
On a desktop it is stored in the app's data folder
(`%APPDATA%\cz.v0v3s.dnt` on Windows), on Android in the app's storage.
