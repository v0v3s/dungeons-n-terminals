// The commands of the Rust side (app/src-tauri/src/lib.rs), typed.
import { invoke } from "@tauri-apps/api/core";

export type Group = "spell" | "class" | "feature" | "species" | "background" | "feat" | "item" | "rule";

export const GLYPH: Record<Group, string> = {
  spell: "✦",
  class: "◆",
  feature: "◇",
  species: "●",
  background: "■",
  feat: "★",
  item: "♦",
  rule: "¶",
};

export interface Status {
  loaded: boolean;
  entries: number;
  sources: [string, number][];
}

export interface Imported {
  summary: string;
  skipped: [string, string][];
  status: Status;
}

export interface Row {
  id: number;
  name: string;
  source: string;
  group: Group;
}

export interface Found {
  rows: Row[];
  total: number;
  categories: [string, number][];
  category: number;
  hints: [string, string][];
  suggestions: [string, number][];
}

export interface Style {
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strike: boolean;
  code: boolean;
  mechanic: boolean;
  muted: boolean;
}

export interface Link {
  tag: string;
  args: string;
}

export interface Span {
  text: string;
  style: Style;
  link: Link | null;
}

export interface Table {
  caption: Span[] | null;
  header: Span[][];
  rows: Span[][][];
}

export type Block =
  | { type: "heading"; c: { level: number; text: Span[] } }
  | { type: "paragraph"; c: Span[] }
  | { type: "list"; c: Block[][] }
  | { type: "table"; c: Table }
  | { type: "inset"; c: { title: Span[] | null; body: Block[] } }
  | { type: "quote"; c: { body: Block[]; by: string | null } }
  | { type: "properties"; c: Span[][] }
  | { type: "rule" };

export interface Entry {
  id: number;
  name: string;
  kind: string;
  group: Group;
  source: string;
  page: number | null;
  blocks: Block[];
}

export const openLibrary = () => invoke<Status>("open_library");
export const importZip = (path: string) => invoke<Imported>("import_zip", { path });
export const search = (category: number, query: string) => invoke<Found>("search", { category, query });
export const entry = (id: number) => invoke<Entry>("entry", { id });
export const follow = (link: Link) => invoke<number | null>("follow", { tag: link.tag, args: link.args });
