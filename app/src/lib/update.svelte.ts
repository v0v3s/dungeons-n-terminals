// Updating dnt: what the last check found, shared by the notice and Data ›
// About.
import * as api from "./api";
import { working } from "./working.svelte";

export const update = $state<{
  /// A newer dnt, when one was found.
  newer: api.Newer | null;
  /// The user asked and it is the newest.
  newest: boolean;
  /// What installing said, or what went wrong.
  message: string;
  error: string;
  /// The notice was closed.
  dismissed: boolean;
}>({ newer: null, newest: false, message: "", error: "", dismissed: false });

/// Check for a newer dnt: `asked` by the user (it says what it found),
/// else the day's quiet check.
export async function checkForUpdate(asked: boolean) {
  update.error = "";
  update.newest = false;
  try {
    const found = asked ? await working("Checking for updates…", () => api.updateCheck(true)) : await api.updateCheck(false);
    if (found) {
      update.newer = found;
      update.dismissed = false;
    } else if (asked) {
      // What an earlier check found is gone (or was installed).
      update.newer = null;
      update.newest = true;
    }
  } catch (e) {
    if (asked) update.error = String(e);
  }
}

/// Download, check and install it.
export async function installUpdate() {
  update.error = "";
  update.message = "";
  try {
    update.message = await working("Downloading the update…", () => api.updateInstall());
  } catch (e) {
    update.error = String(e);
  }
}
