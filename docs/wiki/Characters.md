## Creating one

*+ New character* (in the terminal: **2 Characters**, `n`), a name, then
step by step: species (and a lineage or subrace), background, ability
scores, the increases, an origin feat, the class (and a subclass if it
comes at level 1), skills, a feature's choices, and the starting
equipment.

- Each choice in a list says what it gives: a species its increases and
  speed, a background its skills and feat, a class its hit die, main
  abilities and saves. On a phone, the **?** on a row shows its full text,
  with a button to choose it.
- The ability scores are one screen: pick the method (standard array,
  point buy or your own; `m` in the terminal), then raise or lower each.
- The steps are a checklist with what was chosen in each (on a phone,
  tap *steps* above the question). Tap a finished step to choose it again;
  *Back* changes the last answer.

| | |
| --- | --- |
| ![Choosing a species](images/app-creation.png) | ![The creation wizard](images/tui-creation.svg) |

## The sheet

In the app, six tabs: **Overview** (hit points and hit dice, attacks,
what you track), **Skills** (abilities, saves and skills), **Spells** (by
level, with their slots), **Inventory**, **Features** and **Notes**. The
terminal has five: its **Overview** has the abilities and skills too. Every number
explains itself: hold it on a phone (right-click on a desktop; `enter` in
the terminal) to see how it adds up.

![The overview on a phone](images/app-sheet.png)

The tab select above the sheet changes the tab; on a phone, the row of
icons under the app bar. Swipe sideways too: the gap names the tab you are
going to, and lights up once letting go opens it (a little under half the
way). Unfold *Characters* in the menu (▸) for the open character's tabs:
from the compendium straight to their spells.

On a phone, a `?` tip names each of these gestures once; *Show the tips
again* in the sheet's `⋯` brings them back.

## Levels

*⋯ › Level up* (`u`): the class (the same or another, if your scores allow
it), hit points (average, the maximum, or what you rolled at the table),
the subclass, and an ability score improvement or a feat when they come.
*Remove the latest level* (`U`) undoes it. Choices left open (a character
file edited by hand) are made from the flag on the sheet (`c`).

## The file

A character is a plain TOML file: *⋯ › Edit the character file* (`e`, in
your `$EDITOR`). *Export* writes a printable page or Markdown (`X`).
Deleting asks first.

See also [[Playing]], [[Spells]], [[Inventory]].
