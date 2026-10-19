// What kind of screen the app is on: touch (phones and tablets get the
// mobile frame: app bar, drawer, bigger targets) and narrow (one pane at a
// time). Desktop keeps the TUI-style header and keys.
import { MediaQuery } from "svelte/reactivity";

export const touch = new MediaQuery("(pointer: coarse)");
export const narrow = new MediaQuery("(max-width: 760px)");
