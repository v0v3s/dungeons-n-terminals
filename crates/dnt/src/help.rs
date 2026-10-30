//! The help screen (? or F1): the keys of the current screen first, then
//! the keys that work everywhere, then every other screen.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::characters::Tab;
use crate::theme::Theme;
use crate::ui::{pane, pane_title};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Compendium,
    EntryEditor,
    Campaigns,
    Roster,
    Wizard,
    Sheet(Tab),
    Notes,
}

pub struct Help {
    pub scroll: u16,
    pub context: Context,
}

/// A titled group of keys.
pub struct Group {
    pub title: &'static str,
    pub keys: &'static [(&'static str, &'static str)],
}

const GLOBAL: Group = Group {
    title: "Everywhere",
    keys: &[
        ("1 – 4", "switch section (alt+1–4 while typing)"),
        ("alt+c", "quick capture: a line into the campaign's Inbox note"),
        ("? · F1", "this help (F1 also while typing)"),
        ("q", "quit (from a list; on a sheet it goes back to the roster)"),
        ("ctrl+c", "quit at once"),
    ],
};

const PICKER: Group = Group {
    title: "Pickers and lists of choices",
    keys: &[
        ("type", "filter"),
        ("↑ ↓ · ctrl+n ctrl+p", "move"),
        ("enter", "choose"),
        ("space", "tick or untick (when several can be chosen)"),
        ("esc", "cancel"),
    ],
};

pub fn groups(context: Context) -> Vec<Group> {
    use content::*;
    let mut own: Vec<Group> = match context {
        Context::Compendium => vec![COMPENDIUM_LIST, COMPENDIUM_SEARCH, COMPENDIUM_DETAIL, CLONE_NEW],
        Context::EntryEditor => vec![ENTRY_EDITOR],
        Context::Campaigns => vec![CAMPAIGNS],
        Context::Roster => vec![ROSTER],
        Context::Wizard => vec![WIZARD],
        Context::Sheet(tab) => {
            let tab_group = match tab {
                Tab::Overview => OVERVIEW,
                Tab::Features => FEATURES,
                Tab::Spells => SPELLS,
                Tab::Inventory => INVENTORY,
                Tab::Notes => SHEET_NOTES,
            };
            vec![tab_group, SHEET]
        }
        Context::Notes => vec![NOTES_TREE, NOTES_EDITOR, NOTES_COMMANDS],
    };
    let rest: Vec<Group> = [
        COMPENDIUM_LIST,
        COMPENDIUM_SEARCH,
        COMPENDIUM_DETAIL,
        CLONE_NEW,
        ENTRY_EDITOR,
        CAMPAIGNS,
        ROSTER,
        WIZARD,
        SHEET,
        OVERVIEW,
        FEATURES,
        RULES_EDITOR,
        SPELLS,
        INVENTORY,
        SHEET_NOTES,
        NOTES_TREE,
        NOTES_EDITOR,
        NOTES_COMMANDS,
    ]
    .into_iter()
    .filter(|g| !own.iter().any(|o| o.title == g.title))
    .collect();
    own.push(GLOBAL);
    own.push(PICKER);
    own.extend(rest);
    own
}

fn group_lines(groups: &[Group], key_w: usize, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    for g in groups {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        lines.push(Line::styled(g.title, theme.heading));
        for (k, what) in g.keys {
            lines.push(Line::from(vec![
                Span::styled(format!("  {k:<key_w$}"), Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(what.to_string()),
            ]));
        }
    }
    lines
}

pub fn draw(frame: &mut Frame, area: Rect, help: &mut Help, theme: &Theme) {
    let mut groups = groups(help.context);
    // This screen's groups, then the two shared ones; the rest after.
    let own = groups.iter().position(|g| g.title == GLOBAL.title).map_or(0, |i| i + 2);
    let others = groups.split_off(own.min(groups.len()));
    frame.render_widget(Clear, area);
    let block = pane(pane_title("Help", true, theme), true, theme)
        .padding(Padding::new(3, 3, 1, 0))
        .title_bottom(Line::styled(" j k scroll · d u page · esc close ", theme.dim).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let key_w = groups.iter().chain(&others).flat_map(|g| g.keys.iter()).map(|(k, _)| k.width()).max().unwrap_or(8).min(24) + 3;
    let mut left = vec![Line::styled("This screen", theme.dim.add_modifier(Modifier::ITALIC)), Line::default()];
    left.extend(group_lines(&groups, key_w, theme));
    let mut right = vec![Line::styled("Other screens", theme.dim.add_modifier(Modifier::ITALIC)), Line::default()];
    right.extend(group_lines(&others, key_w, theme));
    right.extend(about(theme));
    if inner.width >= 150 {
        let [l, _, r] = Layout::horizontal([Constraint::Percentage(48), Constraint::Length(4), Constraint::Min(40)]).areas(inner);
        let max = (left.len().max(right.len()) as u16).saturating_sub(inner.height);
        help.scroll = help.scroll.min(max);
        frame.render_widget(Paragraph::new(left).scroll((help.scroll, 0)), l);
        frame.render_widget(Paragraph::new(right).scroll((help.scroll, 0)), r);
    } else {
        left.push(Line::default());
        left.extend(right);
        let max = (left.len() as u16).saturating_sub(inner.height);
        help.scroll = help.scroll.min(max);
        frame.render_widget(Paragraph::new(left).scroll((help.scroll, 0)), inner);
    }
}

/// Who made dnt and on what terms, after the keys.
fn about(theme: &Theme) -> Vec<Line<'static>> {
    let mut out = vec![
        Line::default(),
        Line::styled("About", theme.label),
        Line::raw(format!("dnt {} · MIT license", env!("CARGO_PKG_VERSION"))),
        Line::styled("github.com/v0v3s/dungeons-n-terminals", theme.muted),
        Line::styled("A fan project, not affiliated with Wizards of the Coast.", theme.muted),
        Line::styled("It has the SRD 5.2.1 built in, and reads the books you import.", theme.muted),
        Line::default(),
    ];
    // The SRD's attribution, in lines that fit beside the keys.
    let mut line = String::new();
    for word in dnt_data::srd::ATTRIBUTION.split(' ') {
        if !line.is_empty() && line.width() + word.width() > 60 {
            out.push(Line::styled(std::mem::take(&mut line), theme.muted));
        }
        line = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
    }
    out.push(Line::styled(line, theme.muted));
    out
}

mod content {
    use super::Group;

    pub const COMPENDIUM_LIST: Group = Group {
        title: "Compendium: list",
        keys: &[
            ("j k · ↑ ↓", "move"),
            ("g G", "first · last"),
            ("ctrl+d ctrl+u · pgdn pgup", "half page · page"),
            ("enter · l · →", "read the entry"),
            ("/ · i", "search"),
            ("tab · shift+tab", "next · previous category"),
            ("c", "clone into the campaign (replace the original, or a separate copy)"),
            ("e", "edit a campaign or global homebrew entry"),
            ("n", "new homebrew entry from a template"),
            ("o", "order: best match · name A–Z · Z–A · source · level or rarity"),
            ("esc", "clear the search"),
        ],
    };
    pub const COMPENDIUM_SEARCH: Group = Group {
        title: "Compendium: search",
        keys: &[
            ("type", "fuzzy search; @source #kind *damage ^level \"text\" !not"),
            ("↑ ↓ · ctrl+n ctrl+p", "move through results"),
            ("tab", "complete a filter, else next category"),
            ("ctrl+w · ctrl+u", "delete a word · clear"),
            ("enter", "read the selected entry"),
            ("esc", "back to the list"),
        ],
    };
    pub const COMPENDIUM_DETAIL: Group = Group {
        title: "Compendium: entry",
        keys: &[
            ("j k · space", "scroll · page"),
            ("g G", "top · bottom"),
            ("tab n · shift+tab N", "next · previous link"),
            ("enter", "follow the link"),
            ("bksp · ctrl+o · h", "back"),
            ("o", "the book original of a campaign version, and back"),
            ("c · e", "clone · edit"),
            ("/", "search"),
            ("esc", "back to the list"),
        ],
    };
    pub const CLONE_NEW: Group = Group {
        title: "Clone and create",
        keys: &[
            ("j k · tab", "replace the original, or a separate copy"),
            ("enter", "choose; then type the copy's name"),
            ("j k · enter", "pick a template for a new entry, then name it"),
            ("ctrl+u", "clear the name"),
            ("esc", "back · cancel"),
        ],
    };
    pub const ENTRY_EDITOR: Group = Group {
        title: "Homebrew editor",
        keys: &[
            ("j k · g G", "move between fields"),
            ("enter · i · space", "edit: type it, pick from a list, toggle, or open the text editor"),
            ("J", "edit the field as JSON"),
            ("x", "revert to the book (campaign versions) or clear"),
            ("a", "add a field"),
            ("E", "edit the whole entry in $EDITOR"),
            ("P", "make it global homebrew (every campaign)"),
            ("D", "delete the entry"),
            ("esc · ctrl+s", "save the text editor (in normal mode)"),
            ("esc · q", "close the editor"),
        ],
    };
    pub const CAMPAIGNS: Group = Group {
        title: "Campaigns",
        keys: &[
            ("j k", "move"),
            ("enter", "make the campaign active"),
            ("n", "new campaign (all official sources)"),
            ("r", "rename the campaign"),
            ("D", "delete the campaign (asks first)"),
            ("L", "the rules library: import rules for features"),
            ("tab", "campaigns → books → homebrew"),
            ("l h", "into · out of the books or homebrew"),
            ("space · enter", "turn a book or homebrew source on or off"),
            ("p", "next preset of books"),
            ("a", "all books"),
            ("x", "homebrew: all off"),
            ("r", "show or hide reprinted content"),
            ("/", "filter the books (or the homebrew by name or author)"),
        ],
    };
    pub const ROSTER: Group = Group {
        title: "Characters: roster",
        keys: &[
            ("j k · g G", "move"),
            ("enter · l", "open the sheet"),
            ("n", "new character (the creation wizard)"),
            ("e", "edit the character file in $EDITOR"),
            ("D", "delete the character (asks first; notes stay)"),
        ],
    };
    pub const WIZARD: Group = Group {
        title: "New character",
        keys: &[
            ("type · enter", "pick from each step's list"),
            ("j k · h l · + -", "abilities: move · lower and raise"),
            ("enter", "next step; on the review, create"),
            ("esc", "back a step"),
        ],
    };
    pub const SHEET: Group = Group {
        title: "Character sheet (every tab)",
        keys: &[
            ("tab · shift+tab", "next · previous tab"),
            ("d · h · t", "take damage · heal (when dead: not dead) · temporary hit points"),
            ("D", "roll a death save (at 0 hit points)"),
            ("s · L", "short rest · long rest"),
            ("u · U", "level up · undo the last level"),
            ("c", "make pending choices"),
            ("e", "edit the character file in $EDITOR"),
            ("X", "export as Markdown and printable HTML"),
            ("esc · q", "back to the roster"),
        ],
    };
    pub const OVERVIEW: Group = Group {
        title: "Sheet: Overview",
        keys: &[
            ("j k · g G", "move"),
            ("enter", "how a number is made up; on a toggle, turn it on or off"),
            ("r", "roll the check, skill, initiative or attack under the cursor; on a hit die, spend it and heal"),
            ("R", "roll the saving throw of an ability"),
            ("v", "advantage, then disadvantage, for the next roll"),
            ("C", "conditions and exhaustion"),
            ("N", "also write rolls into the latest session note"),
            ("space · -", "use a resource, spell slot or hit die (without rolling)"),
            ("+", "restore one"),
            ("a · x", "a tracker of your own (name, uses, recharge) · remove yours"),
            ("space · w (attack)", "hold in the main hand, or put away"),
            ("o · b (attack)", "off hand · both hands"),
            ("S", "take up or put away the shield"),
        ],
    };
    pub const FEATURES: Group = Group {
        title: "Sheet: Features",
        keys: &[
            ("j k", "move"),
            ("J K · ctrl+d", "scroll the text"),
            ("O", "write or change the feature's rules"),
            ("r", "swap a Weapon Mastery"),
        ],
    };
    pub const RULES_EDITOR: Group = Group {
        title: "Rules editor (O on a feature)",
        keys: &[
            ("i · esc", "insert · normal mode (vim keys)"),
            ("esc · ctrl+s", "save (from normal mode; not while there are problems)"),
            ("ctrl+q", "close without saving"),
        ],
    };
    pub const SPELLS: Group = Group {
        title: "Sheet: Spells",
        keys: &[
            ("j k · g G", "move"),
            ("a", "add spells for the class under the cursor"),
            ("enter", "cast (choose a slot) · choose spells for a feature"),
            ("r", "roll the spell's attack and damage"),
            ("p", "prepare or unprepare (spellbook)"),
            ("f", "favorite: listed first"),
            ("x", "remove"),
            ("C", "stop concentrating"),
        ],
    };
    pub const INVENTORY: Group = Group {
        title: "Sheet: Inventory",
        keys: &[
            ("j k · G", "move"),
            ("a", "add: from the books, a new campaign item, or a custom one"),
            ("space", "wear (choose a slot), take off, or hold a weapon or shield"),
            ("o", "hold in the off hand"),
            ("A", "attune"),
            ("+ -", "quantity"),
            ("x", "remove (a container's contents stay)"),
            ("M", "put into a container, or carry loose"),
            ("enter · ← →", "fold and unfold containers"),
            ("p", "unpack an equipment pack"),
            ("w", "container settings: 30 · weightless · no limit · off"),
            ("m", "mark or unmark a monk weapon"),
            ("g", "coins: +15gp, -3sp, 20"),
        ],
    };
    pub const SHEET_NOTES: Group = Group { title: "Sheet: Notes", keys: &[("enter · i", "open the character's note")] };
    pub const NOTES_TREE: Group = Group {
        title: "Notes: tree",
        keys: &[
            ("j k · g G", "move"),
            ("enter · l · →", "open a note, or a folder"),
            ("h · ←", "close a folder, or go to the parent"),
            ("a · A", "new note · new folder"),
            ("s", "new session note"),
            ("r · d", "rename · delete"),
            ("J K", "move a top-level folder"),
            ("/ · ctrl+p", "find notes: words #tag \"text\" key:value @Folder !not"),
            ("i · c", "Inbox · the campaign hub"),
            ("tab", "back into the open note"),
        ],
    };
    pub const NOTES_EDITOR: Group = Group {
        title: "Notes: editor",
        keys: &[
            ("vim keys", "edit; i insert, esc normal"),
            ("esc (normal)", "save and go back to the tree"),
            ("ctrl+s", "save"),
            ("gd", "follow the [[link]] under the cursor"),
            ("K", "preview the link"),
            ("ctrl+o", "back to the previous note"),
            (":", "command"),
        ],
    };
    pub const NOTES_COMMANDS: Group = Group {
        title: "Notes: commands (:)",
        keys: &[
            (":w", "save"),
            (":q · :wq · :x", "save and close"),
            (":e <note>", "open a note"),
            (":new <name>", "new note in this folder"),
            (":session · :inbox · :hub", "new session note · Inbox · the campaign hub"),
            (":find <query>", "search"),
        ],
    };
}
