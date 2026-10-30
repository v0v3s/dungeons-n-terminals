package cz.v0v3s.dnt

// Installing an update (src-tauri/src/updates.rs): Android's own installer
// opens on the downloaded APK, and the user confirms. Android installs it
// only when it is signed like the app itself. An app may ask to install
// only once the user allowed it ("Install unknown apps"), which this opens.

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class InstallArgs {
  lateinit var path: String
}

@TauriPlugin
class InstallPlugin(private val activity: Activity) : Plugin(activity) {
  /// Whether the user allowed dnt to install apps.
  @Command
  fun canInstall(invoke: Invoke) {
    val ret = JSObject()
    ret.put("allowed", activity.packageManager.canRequestPackageInstalls())
    invoke.resolve(ret)
  }

  /// The system's page where the user allows it.
  @Command
  fun allowInstalls(invoke: Invoke) {
    val intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + activity.packageName))
    activity.startActivity(intent)
    invoke.resolve()
  }

  /// The installer, on an APK in the app's cache.
  @Command
  fun install(invoke: Invoke) {
    val args = invoke.parseArgs(InstallArgs::class.java)
    val uri = FileProvider.getUriForFile(activity, activity.packageName + ".fileprovider", File(args.path))
    val intent = Intent(Intent.ACTION_VIEW)
    intent.setDataAndType(uri, "application/vnd.android.package-archive")
    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    activity.startActivity(intent)
    invoke.resolve()
  }
}
