// The left sidebar of a section on a computer (the notes' folders, later
// the party and the data's pages): the header's ☰ hides and shows it. It
// starts hidden in a window too narrow for it and a page beside it, and
// stays as the user leaves it from then on.
import { MediaQuery } from "svelte/reactivity";

/// Wide enough for a sidebar and two columns beside it.
const roomy = new MediaQuery("(min-width: 1200px)");

/// What the user chose with ☰, if anything yet.
let chosen = $state<boolean | null>(null);

export const sidebar = {
  /// Whether the sidebar is shown.
  get open(): boolean {
    return chosen ?? roomy.current;
  },
  toggle() {
    chosen = !this.open;
  },
};
