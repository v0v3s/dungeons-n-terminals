Every [release](https://github.com/v0v3s/dungeons-n-terminals/releases/latest)
has the terminal app and the app, with `SHA256SUMS` to check the downloads.

## The terminal app

**Linux** (x86_64):

```sh
curl -LO https://github.com/v0v3s/dungeons-n-terminals/releases/latest/download/dnt-linux-x86_64
chmod +x dnt-linux-x86_64 && mv dnt-linux-x86_64 ~/.local/bin/dnt
```

**Windows**: download `dnt-windows-x86_64.exe` (rename it to `dnt.exe` if
you like) and run it in Windows Terminal.

**From source**, with [Rust](https://rustup.rs):

```sh
git clone https://github.com/v0v3s/dungeons-n-terminals.git
cd dungeons-n-terminals
cargo build --release      # the binary is target/release/dnt
```

## The app

- **Windows**: `dnt-app-windows-x86_64-setup.exe`, an installer. It is not
  code-signed, so SmartScreen asks first: *More info* › *Run anyway*.
- **Android**: `dnt-app-android.apk`. Open it on the device and allow
  installing from that app (or `adb install -r dnt-app-android.apk`). Newer
  releases install over older ones.

The terminal app and the app keep their own campaigns: they do not share
them. Next: [[Getting started]].
