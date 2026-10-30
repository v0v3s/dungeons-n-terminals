//! Updating the app: whether there is a newer dnt (by itself once a day,
//! or when asked), and installing it: on Android, Android's installer on
//! the downloaded APK (InstallPlugin.kt); on Windows, the installer run.

use dnt_core::update::{self, Target};
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use crate::{App, Res};

/// Register the Android side.
pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("install")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api.register_android_plugin("cz.v0v3s.dnt", "InstallPlugin")?;
                _app.manage(Plugin(handle));
            }
            Ok(())
        })
        .build()
}

#[cfg(target_os = "android")]
pub struct Plugin(pub tauri::plugin::PluginHandle<tauri::Wry>);

#[derive(Serialize)]
pub struct UpdateState {
    /// This dnt's version.
    current: String,
    /// Whether it checks by itself, once a day.
    auto: bool,
}

#[derive(Serialize)]
pub struct Newer {
    version: String,
    /// Its release notes (Markdown).
    notes: String,
}

fn state_of(app: &App) -> UpdateState {
    UpdateState { current: update::VERSION.to_string(), auto: app.lock().ws.config.update.auto }
}

#[tauri::command]
pub async fn update_state(app: State<'_, App>) -> Res<UpdateState> {
    Ok(state_of(&app))
}

/// Turn the daily check on or off.
#[tauri::command]
pub async fn update_auto(app: State<'_, App>, on: bool) -> Res<UpdateState> {
    {
        let mut inner = app.lock();
        inner.ws.config.update.auto = on;
        inner.ws.store.save_config(&inner.ws.config).map_err(|e| e.to_string())?;
    }
    Ok(state_of(&app))
}

/// Where the releases are: the tests' (e2e), else GitHub's newest.
fn location(app: &App) -> String {
    app.lock().update_test.clone().unwrap_or_else(update::location)
}

/// A newer dnt, if there is one. `asked`: the user asked (errors are
/// said); else the day's automatic check (once a day, quietly).
#[tauri::command]
pub async fn update_check(app: State<'_, App>, asked: bool) -> Res<Option<Newer>> {
    if !asked {
        let mut inner = app.lock();
        // The tests never ask GitHub by themselves: only their releases.
        if cfg!(feature = "e2e") && inner.update_test.is_none() {
            return Ok(None);
        }
        let today = dnt_core::backup::now()[..10].to_string();
        if !update::due(&inner.ws.config.update, &today) {
            return Ok(None);
        }
        inner.ws.config.update.checked = Some(today);
        let _ = inner.ws.store.save_config(&inner.ws.config);
    }
    let at = location(&app);
    let found = tauri::async_runtime::spawn_blocking(move || update::latest(&at)).await.map_err(|e| e.to_string())?;
    let latest = match found {
        Ok(l) => l,
        Err(e) if asked => return Err(e),
        Err(_) => return Ok(None),
    };
    Ok(update::newer(&latest.version, update::VERSION).then_some(Newer { version: latest.version, notes: latest.notes }))
}

#[derive(Deserialize)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
struct Allowed {
    allowed: bool,
}

/// Download the newer dnt for this system, check it, and install it:
/// what happens next, to say.
#[tauri::command]
pub async fn update_install(handle: tauri::AppHandle, app: State<'_, App>) -> Res<String> {
    let target = if cfg!(target_os = "android") {
        Target::Android
    } else if cfg!(windows) {
        Target::WindowsApp
    } else {
        return Err("Update from github.com/v0v3s/dungeons-n-terminals/releases".into());
    };
    let test = app.lock().update_test.is_some();
    #[cfg(target_os = "android")]
    if !test {
        // Android asks the user once to let dnt install apps.
        let plugin = handle.state::<Plugin>();
        let can: Allowed = plugin.0.run_mobile_plugin("canInstall", ()).map_err(|e| e.to_string())?;
        if !can.allowed {
            plugin.0.run_mobile_plugin::<()>("allowInstalls", ()).map_err(|e| e.to_string())?;
            return Ok("Allow dnt to install apps on the page that opened, then come back and update again".into());
        }
    }
    let at = location(&app);
    let dir = handle.path().app_cache_dir().map_err(|e| e.to_string())?.join("updates");
    let (version, file) = tauri::async_runtime::spawn_blocking(move || -> Res<(String, std::path::PathBuf)> {
        let latest = update::latest(&at)?;
        let file = update::download(&latest, target, &dir)?;
        Ok((latest.version, file))
    })
    .await
    .map_err(|e| e.to_string())??;
    let name = target.asset();
    if test {
        // The tests stop short of the system's installer.
        return Ok(format!("dnt {version} downloaded and checked ({name}); the tests do not install it"));
    }
    #[cfg(target_os = "android")]
    {
        #[derive(Serialize)]
        struct Args {
            path: String,
        }
        let plugin = handle.state::<Plugin>();
        plugin.0.run_mobile_plugin::<()>("install", Args { path: file.display().to_string() }).map_err(|e| e.to_string())?;
        Ok(format!("dnt {version}: confirm the update in Android's installer"))
    }
    #[cfg(not(target_os = "android"))]
    {
        std::process::Command::new(&file).spawn().map_err(|e| format!("Could not run the installer: {e}"))?;
        // The installer replaces the app: out of its way.
        let h = handle.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            h.exit(0);
        });
        Ok(format!("dnt {version}: the installer is starting"))
    }
}

/// The tests' releases (e2e/update/<one>): its files, as text.
#[tauri::command]
pub async fn e2e_update_source(handle: tauri::AppHandle, app: State<'_, App>, files: Vec<(String, String)>) -> Res<()> {
    crate::e2e::enabled()?;
    let dir = handle.path().app_cache_dir().map_err(|e| e.to_string())?.join("e2e-update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    for (name, text) in files {
        std::fs::write(dir.join(name), text).map_err(|e| e.to_string())?;
    }
    app.lock().update_test = Some(dir.display().to_string());
    Ok(())
}
