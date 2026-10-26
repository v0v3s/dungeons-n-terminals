// Swiping sideways between views: a sheet's tabs, the compendium's
// categories. The view follows a sideways drag, and the gap it leaves names
// the next (or previous) one (see SwipeHint.svelte); let go past SWITCH of
// the width, or with a flick, and that one opens, otherwise it slides back.

const EDGE = 40;
const SWITCH = 0.45;

export class Swipe<T> {
  /// How far the view is dragged (px), the view beside it, and whether it
  /// is sliding to rest.
  drag = $state(0);
  beside: T | null = $state(null);
  settling = $state(false);
  /// Whether letting go now opens the view beside.
  ready = $state(false);
  /// The body's visible part when the drag began (its scroll and height):
  /// where the hint shows.
  top = $state(0);
  height = $state(0);

  #items: () => readonly T[];
  #current: () => T;
  #open: (t: T) => void;
  #body: () => HTMLElement | undefined;
  /// The touch being followed; swipes from the screen's edges are the system's.
  #gesture: { x: number; y: number; lastX: number; lastT: number; speed: number; axis: "x" | "y" | null } | null = null;
  #width = 1;

  constructor(o: { items: () => readonly T[]; current: () => T; open: (t: T) => void; body: () => HTMLElement | undefined }) {
    this.#items = o.items;
    this.#current = o.current;
    this.#open = o.open;
    this.#body = o.body;
  }

  #besideOf(dx: number): T | null {
    const items = this.#items();
    const at = items.indexOf(this.#current());
    return at < 0 ? null : (items[at + (dx < 0 ? 1 : -1)] ?? null);
  }

  start = (e: TouchEvent) => {
    const p = e.touches[0];
    const inside = e.touches.length === 1 && p.clientX > EDGE && p.clientX < window.innerWidth - EDGE;
    this.#gesture = inside && !this.settling ? { x: p.clientX, y: p.clientY, lastX: p.clientX, lastT: e.timeStamp, speed: 0, axis: null } : null;
  };

  move = (e: TouchEvent) => {
    const g = this.#gesture;
    const p = e.touches[0];
    if (!g || !p) return;
    const dx = p.clientX - g.x;
    const dy = p.clientY - g.y;
    if (!g.axis) {
      // A clear sideways move drags the view; anything else scrolls.
      if (Math.hypot(dx, dy) < 12) return;
      g.axis = Math.abs(dx) > 1.5 * Math.abs(dy) ? "x" : "y";
      const body = this.#body();
      this.#width = body?.clientWidth || 1;
      this.top = body?.scrollTop ?? 0;
      this.height = body?.clientHeight ?? 0;
    }
    if (g.axis !== "x") return;
    if (e.timeStamp > g.lastT) g.speed = (p.clientX - g.lastX) / (e.timeStamp - g.lastT);
    g.lastX = p.clientX;
    g.lastT = e.timeStamp;
    this.beside = this.#besideOf(dx);
    // Nothing beside: it gives a little and comes back.
    this.drag = this.beside !== null ? dx : dx / 4;
    this.ready = this.beside !== null && Math.abs(dx) >= SWITCH * this.#width;
  };

  end = (e: TouchEvent) => {
    const g = this.#gesture;
    this.#gesture = null;
    if (!g || g.axis !== "x") return;
    const far = Math.abs(this.drag) >= SWITCH * this.#width;
    // A finger held still before it lifts does not flick.
    const moving = e.timeStamp - g.lastT < 100;
    const flick = moving && Math.abs(g.speed) > 0.6 && Math.sign(g.speed) === Math.sign(this.drag) && Math.abs(this.drag) > 0.2 * this.#width;
    this.#settle(this.beside !== null && (far || flick) ? Math.sign(this.drag) * this.#width : 0);
  };

  #settle(to: number) {
    if (Math.abs(this.drag - to) < 1) return this.settled();
    this.settling = true;
    this.drag = to;
  }

  /// The slide came to rest (its transitionend).
  settled = (e?: TransitionEvent) => {
    if (e && e.target !== e.currentTarget) return;
    this.settling = false;
    if (this.drag !== 0 && this.beside !== null) this.#open(this.beside);
    this.drag = 0;
    this.beside = null;
    this.ready = false;
  };
}
