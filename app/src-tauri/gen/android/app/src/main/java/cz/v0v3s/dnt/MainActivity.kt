package cz.v0v3s.dnt

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Edge to edge, the keyboard overlays the page. Shrink the page by the
    // keyboard's height instead, so nothing hides under it and the page
    // sees the resize (the search's extra keys follow the keyboard).
    val root = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(0, 0, 0, ime.bottom)
      insets
    }
  }
}
