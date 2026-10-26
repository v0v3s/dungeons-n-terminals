// The system's file dialogs. The end-to-end tests answer them instead
// (nothing can drive the system's own dialogs): see app/e2e/fixtures.ts and
// src-tauri/src/e2e.rs.
import { open, save } from "@tauri-apps/plugin-dialog";

type Answers = { open?: string | null; save?: string | null };

/// A test's answers, when one set them.
const answers = (): Answers | undefined => (window as { dntE2eFiles?: Answers }).dntE2eFiles;

/// Pick a zip to import: its path (a `content://` URI on Android), or null.
export async function openZip(): Promise<string | null> {
  const a = answers();
  if (a && "open" in a) return a.open ?? null;
  const path = await open({ multiple: false, directory: false, filters: [{ name: "Zip", extensions: ["zip"] }] });
  return typeof path === "string" ? path : null;
}

/// Where to save a file named `name` (of the kind `label`), or null.
export async function saveAs(name: string, label: string, ext: string): Promise<string | null> {
  const a = answers();
  if (a && "save" in a) return a.save ? `${a.save}${name}` : null;
  return save({ defaultPath: name, filters: [{ name: label, extensions: [ext] }] });
}
