// What the sheet's tabs can ask of the sheet.
import type { Command, Options, Roll, Wanted } from "./api";

export interface SheetCtx {
  /// Apply a command; true when it worked.
  act: (cmd: Command) => Promise<boolean>;
  roll: (what: Roll) => void;
  /// Roll dice typed by hand (`d20 + prof + dex`).
  dice: () => void;
  /// Choose from a list, then do something with the values.
  pick: (wanted: Wanted, then: (values: string[], options: Options) => void) => void;
  /// Choose from a list of actions (label, run).
  menu: (title: string, actions: [string, () => void][]) => void;
  /// Ask before doing something (`danger` names a destructive action).
  confirm: (title: string, text: string, then: () => void, danger?: string) => void;
  /// Ask for a line of text (or a number).
  prompt: (title: string, then: (value: string) => void, opts?: { number?: boolean; value?: string; hint?: string }) => void;
  /// Show a compendium entry, or one part of it (a species trait).
  show: (entity: number, part?: string | null) => void;
  /// Write or change a feature's rules.
  rules: (entity: number) => void;
  /// How a stat is made up: on a computer in a pop-up beside `near` (the
  /// control it was asked from), elsewhere in a dialog.
  explain: (key: string, near?: Element | null) => void;
  /// Normal → advantage → disadvantage for the next roll.
  cycle: () => void;
  /// Say something in the status line.
  say: (message: string) => void;
  /// Read the sheet again (after something changed it from outside).
  refresh: () => Promise<void>;
}

/// A pool for the `spend` command from a tracker id.
export function pool(id: string): Record<string, unknown> {
  const [kind, arg] = id.split(":");
  if (kind === "resource") return { pool: "resource", key: arg };
  if (kind === "slot") return { pool: "slot", level: Number(arg) };
  if (kind === "hit_die") return { pool: "hit_die", faces: Number(arg) };
  return { pool: "pact" };
}

/// The sheet's tabs; Notes opens the character's note.
export const SHEET_TABS = ["Overview", "Skills", "Spells", "Inventory", "Features", "Notes"] as const;
export type SheetTab = (typeof SHEET_TABS)[number];
