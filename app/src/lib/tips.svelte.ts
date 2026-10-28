// One-time tips on a phone: a line naming a gesture the screen does not
// show (swiping, holding). A tip goes once dismissed, or once the gesture
// is used; the sheet's ⋯ menu brings them back. Kept on this device.

const KEY = "dnt.tips";

function load(): string[] {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "[]");
  } catch {
    return [];
  }
}

class Tips {
  #seen: string[] = $state(load());

  /// Whether tip `id` is still to be shown.
  due(id: string) {
    return !this.#seen.includes(id);
  }

  /// Tip `id` was read, or its gesture used.
  done(id: string) {
    if (!this.due(id)) return;
    this.#seen = [...this.#seen, id];
    this.#save();
  }

  /// Show every tip again.
  reset() {
    this.#seen = [];
    this.#save();
  }

  #save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.#seen));
    } catch {
      // Not kept: it is gone for now.
    }
  }
}

export const tips = new Tips();
