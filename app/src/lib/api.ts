// The commands of the Rust side (app/src-tauri/src), typed.
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

// ---- data and campaigns ----

export interface CampaignInfo {
  slug: string;
  name: string;
  sources: string[];
  homebrew: string[];
  show_reprinted: boolean;
  source: string;
  active: boolean;
}

export interface Startup {
  ready: boolean;
  stage: string;
}

export const startup = () => invoke<Startup>("startup");

export interface Status {
  loaded: boolean;
  entries: number;
  imported: [string, number][];
  /// Imported homebrew: sources, entities.
  homebrew: [number, number];
  campaigns: CampaignInfo[];
  campaign: string | null;
  problems: string[];
  /// How the last load went: stage, milliseconds.
  timings: [string, number][];
}

export interface Imported {
  summary: string;
  skipped: [string, string][];
  status: Status;
}

export interface Source {
  id: string;
  name: string;
  group: string;
  published: string | null;
}

export interface BrewSource {
  id: string;
  name: string;
  authors: string[];
  /// `one` for homebrew written for the 2024 rules.
  edition: string;
  kinds: [string, number][];
}

export interface Sources {
  sources: Source[];
  presets: [string, string[]][];
  homebrew: BrewSource[];
}

export const status = () => invoke<Status>("status");
export const importZip = (path: string) => invoke<Imported>("import_zip", { path });
export const sources = () => invoke<Sources>("sources");
export const createCampaign = (name: string, sources: string[]) => invoke<Status>("create_campaign", { name, sources });
export const activateCampaign = (slug: string) => invoke<Status>("activate_campaign", { slug });
export const setCampaignSources = (slug: string, sources: string[], homebrew: string[], showReprinted: boolean) =>
  invoke<Status>("set_campaign_sources", { slug, sources, homebrew, showReprinted });
export const clearImported = (homebrewOnly: boolean) => invoke<Status>("clear_imported", { homebrewOnly });
export const resetApp = () => invoke<Status>("reset_app");
export const deleteCampaign = (slug: string) => invoke<Status>("delete_campaign", { slug });
export const renameCampaign = (slug: string, name: string) => invoke<Status>("rename_campaign", { slug, name });

// ---- compendium ----

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

export const search = (category: number, query: string) => invoke<Found>("search", { category, query });
export const entry = (id: number) => invoke<Entry>("entry", { id });
export const follow = (link: Link) => invoke<number | null>("follow", { tag: link.tag, args: link.args });

// ---- characters ----

export interface RosterRow {
  slug: string;
  name: string;
  identity: string;
}

export interface Header {
  name: string;
  identity: string;
  level: number;
  pending: number;
  spells_to_choose: number;
  warnings: string[];
}

export interface Ability {
  key: string;
  name: string;
  score: string;
  modifier: string;
  save: string;
  proficient: boolean;
}

export interface Skill {
  key: string;
  name: string;
  ability: string;
  proficiency: "none" | "half" | "proficient" | "expertise";
  bonus: string;
}

export interface Tracker {
  id: string;
  name: string;
  left: number;
  max: number;
  recharge: string;
  stat: string;
}

export interface AttackRow {
  slug: string;
  name: string;
  hand: "" | "main" | "off" | "both" | "pair";
  hit: string;
  proficient: boolean;
  damage: string;
  range: string;
  notes: string[];
  problems: string[];
  properties: string[];
  line: number | null;
}

export interface Rolled {
  title: string;
  result: string;
  detail: string;
  extra: string[];
  crit: boolean | null;
}

export interface Condition {
  name: string;
  summary: string;
}

export interface Overview {
  abilities: Ability[];
  casting: { name: string; dc: string; attack: string }[];
  skills: Skill[];
  tiles: { key: string; label: string; value: string }[];
  hp: { current: number; max: number; temp: number; death_successes: number; death_failures: number };
  conditions: Condition[];
  exhaustion: number;
  exhaustion_summary: string;
  concentration: string | null;
  rolls: Rolled[];
  next_roll: "normal" | "advantage" | "disadvantage";
  trackers: Tracker[];
  toggles: { key: string; name: string; on: boolean }[];
  held: string[];
  attacks: AttackRow[];
  riders: { value: string; name: string; when: string }[];
}

export interface Feature {
  index: number;
  entity: number;
  name: string;
  level: number;
  automated: boolean;
  mastery: string | null;
}

export interface Target {
  kind: "class" | "grant";
  class_id?: string;
  key?: string;
}

export type SpellRow =
  | { type: "header"; title: string; detail: string; counts: [string, number, number][]; class_id: string | null }
  | {
      type: "spell";
      id: string;
      entity: number | null;
      name: string;
      mark: "cantrip" | "prepared" | "book" | "granted";
      detail: string;
      favorite: boolean;
      castable: boolean;
      free: boolean;
      target: Target | null;
      class_id: string | null;
      in_favorites: boolean;
    }
  | { type: "choice"; key: string; filter: string; missing: number; text: string };

export interface Line {
  index: number;
  depth: number;
  name: string;
  entity: number | null;
  quantity: number;
  weight: number;
  kind: string;
  worn: boolean;
  held: boolean;
  attuned: boolean;
  needs_attunement: boolean;
  fill: string | null;
  over: boolean;
  weightless: boolean;
}

export interface Inventory {
  lines: Line[];
  carried: number;
  capacity: number;
  attuned: number;
  coins: [string, number][];
  worn: { slot: string; label: string; items: [number, string][] }[];
  main_hand: [number, string] | null;
  off_hand: [number, string] | null;
}

export interface Sheet {
  slug: string;
  header: Header;
  overview: Overview;
  features: { title: string; features: Feature[] }[];
  spells: SpellRow[];
  inventory: Inventory;
}

export interface Outcome {
  message: string;
  changed: boolean;
  rolled: Rolled | null;
}

export interface Done {
  outcome: Outcome;
  sheet: Sheet;
}

export interface Choice {
  label: string;
  hint: string;
  warning: string | null;
  value: string;
}

export interface Options {
  title: string;
  choices: Choice[];
  pick: number;
  default: number;
  target: Target | null;
}

export interface Part {
  kind: string;
  value: string;
  origin: string;
  note: string | null;
  applied: boolean;
}

export interface Breakdown {
  title: string;
  value: string;
  parts: Part[];
}

/// A dnt-core command (see crates/dnt-core/src/commands.rs).
export type Command = { command: string; [field: string]: unknown };

export type Roll =
  | { type: "ability"; key: string }
  | { type: "save"; key: string }
  | { type: "skill"; key: string }
  | { type: "initiative" }
  | { type: "attack"; slug: string }
  | { type: "spell"; id: string };

export type Wanted =
  | { type: "items" }
  | { type: "wear"; line: number }
  | { type: "containers"; line: number }
  | { type: "class_spells"; class_id: string }
  | { type: "grant_spells"; key: string; filter: string; missing: number }
  | { type: "cast"; id: string }
  | { type: "mastery" };

export type Question =
  | { type: "pick"; title: string; choices: Choice[]; count: number }
  | { type: "number"; title: string }
  | { type: "abilities"; scores: { method: "standard_array" | "point_buy" | "manual"; scores: number[] } }
  | { type: "confirm" };

export type Answer = { type: "pick"; value: string[] } | { type: "number"; value: number } | { type: "scores"; value: number[] };

export interface Step {
  question: Question;
  summary: string[];
}

export interface CreationStep {
  question: Question;
  summary: [string, string][];
  progress: [string, boolean, boolean][];
  name: string;
}

export interface Preview {
  entity: Entry | null;
  equipment: [string, string[]] | null;
  identity: string;
  stats: [string, string][];
  abilities: [string, string, string][];
  warnings: string[];
}

export interface PendingChoice {
  level: number;
  key: string;
  from: string;
  left: number;
  question: Question;
}

export const roster = () => invoke<RosterRow[]>("roster");
export const openCharacter = (slug: string) => invoke<Sheet>("open_character", { slug });
export const closeCharacter = () => invoke<void>("close_character");
export const sheet = () => invoke<Sheet>("sheet");
export const command = (command: Command) => invoke<Done>("command", { command });
export const roll = (what: Roll) => invoke<{ line: string; rolled: Rolled; sheet: Sheet }>("roll", { what });
export const cycleAdvantage = () => invoke<Sheet>("cycle_advantage");
export const breakdown = (key: string) => invoke<Breakdown | null>("breakdown", { key });
export const conditions = () => invoke<[Condition, boolean][]>("conditions");
export const options = (wanted: Wanted) => invoke<Options>("options", { wanted });
export const pending = () => invoke<PendingChoice | null>("pending");
export const levelupStart = () => invoke<Step>("levelup_start");
export const levelupAnswer = (answer: Answer) => invoke<Step>("levelup_answer", { answer });
export const levelupApply = () => invoke<Done>("levelup_apply");
export const creationStart = (name: string) => invoke<CreationStep>("creation_start", { name });
export const creationAnswer = (answer: Answer) => invoke<CreationStep>("creation_answer", { answer });
export const creationBack = () => invoke<CreationStep | null>("creation_back");
export const creationPreview = (value: string | null, scores: number[] | null) =>
  invoke<Preview>("creation_preview", { value, scores });
export const creationFinish = () => invoke<Sheet>("creation_finish");

// ---- notes ----

export interface NoteRow {
  path: string;
  name: string;
  is_dir: boolean;
  depth: number;
}

export interface Note {
  path: string;
  title: string;
  text: string;
  backlinks: [string, string][];
}

export type NoteLink =
  | { type: "entity"; id: number }
  | { type: "note"; path: string }
  | { type: "missing"; title: string; compendium: boolean };

export const noteTree = (expanded: string[]) => invoke<NoteRow[]>("note_tree", { expanded });
export const noteRead = (path: string) => invoke<Note>("note_read", { path });
export const noteWrite = (path: string, text: string) => invoke<void>("note_write", { path, text });
export const noteCreate = (folder: string, title: string) => invoke<string>("note_create", { folder, title });
export const noteCreateFolder = (parent: string, name: string) => invoke<string>("note_create_folder", { parent, name });
export const noteRename = (path: string, name: string) => invoke<string>("note_rename", { path, name });
export const noteDelete = (path: string) => invoke<void>("note_delete", { path });
export const noteSearch = (query: string) => invoke<[string, string][]>("note_search", { query });
export const noteResolve = (target: string) => invoke<NoteLink>("note_resolve", { target });
export const noteCapture = (text: string) => invoke<string>("note_capture", { text });
export const noteNewSession = () => invoke<string>("note_new_session");
export const noteForCharacter = (name: string) => invoke<string>("note_character_note", { name });
