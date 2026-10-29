package cz.v0v3s.dnt

// A folder the user picked for backups (src-tauri/src/folders.rs): the
// system's folder picker, and the files in the folder it granted. Files
// are copied to and from files of the app's own, which Rust reads and
// writes.

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class TreeArgs {
  lateinit var tree: String
}

@InvokeArg
class FileArgs {
  lateinit var tree: String
  lateinit var name: String
  var path: String = ""
}

@TauriPlugin
class FoldersPlugin(private val activity: Activity) : Plugin(activity) {
  private val access = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

  @Command
  fun pickFolder(invoke: Invoke) {
    val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
    intent.addFlags(access or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
    startActivityForResult(invoke, intent, "pickFolderResult")
  }

  @ActivityCallback
  fun pickFolderResult(invoke: Invoke, result: ActivityResult) {
    val tree = result.data?.data
    val out = JSObject()
    if (result.resultCode == Activity.RESULT_OK && tree != null) {
      // Kept after a restart.
      activity.contentResolver.takePersistableUriPermission(tree, access)
      out.put("tree", tree.toString())
    }
    invoke.resolve(out)
  }

  /// The folder's files: name, size and where.
  private fun files(tree: Uri): List<Triple<String, Long, Uri>> {
    val kids = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
    val columns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_SIZE, Document.COLUMN_MIME_TYPE)
    val out = mutableListOf<Triple<String, Long, Uri>>()
    activity.contentResolver.query(kids, columns, null, null, null)?.use { c ->
      while (c.moveToNext()) {
        if (c.getString(3) == Document.MIME_TYPE_DIR) continue
        out.add(Triple(c.getString(1), c.getLong(2), DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(0))))
      }
    }
    return out
  }

  private fun find(args: FileArgs): Uri? = files(Uri.parse(args.tree)).firstOrNull { it.first == args.name }?.third

  /// Off the main thread: copies can be large.
  private fun background(invoke: Invoke, work: () -> JSObject) {
    Thread {
      try {
        invoke.resolve(work())
      } catch (e: Exception) {
        invoke.reject(e.message ?: e.toString())
      }
    }.start()
  }

  @Command
  fun list(invoke: Invoke) {
    val args = invoke.parseArgs(TreeArgs::class.java)
    background(invoke) {
      val list = JSArray()
      for ((name, size, _) in files(Uri.parse(args.tree))) {
        val file = JSObject()
        file.put("name", name)
        file.put("size", size)
        list.put(file)
      }
      val out = JSObject()
      out.put("files", list)
      out
    }
  }

  /// The app's file `path` into the folder as `name`.
  @Command
  fun copyIn(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    background(invoke) {
      val tree = Uri.parse(args.tree)
      val doc = find(args)
        ?: DocumentsContract.createDocument(
          activity.contentResolver,
          DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree)),
          "application/zip",
          args.name,
        )
        ?: throw Exception("Could not create ${args.name}")
      activity.contentResolver.openOutputStream(doc, "wt")!!.use { out ->
        File(args.path).inputStream().use { it.copyTo(out) }
      }
      JSObject()
    }
  }

  /// The folder's `name` into the app's file `path`.
  @Command
  fun copyOut(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    background(invoke) {
      val doc = find(args) ?: throw Exception("No ${args.name} in the folder")
      activity.contentResolver.openInputStream(doc)!!.use { input ->
        File(args.path).outputStream().use { input.copyTo(it) }
      }
      JSObject()
    }
  }

  @Command
  fun remove(invoke: Invoke) {
    val args = invoke.parseArgs(FileArgs::class.java)
    background(invoke) {
      val doc = find(args) ?: throw Exception("No ${args.name} in the folder")
      DocumentsContract.deleteDocument(activity.contentResolver, doc)
      JSObject()
    }
  }
}
