fn main() {
    // An app store's build (DNT_STORE=1, see app/README.md): the store
    // updates dnt, so it has no updater of its own.
    println!("cargo::rustc-check-cfg=cfg(store)");
    println!("cargo::rerun-if-env-changed=DNT_STORE");
    if std::env::var("DNT_STORE").is_ok_and(|v| !v.is_empty() && v != "0") {
        println!("cargo::rustc-cfg=store");
    }
    tauri_build::build()
}
