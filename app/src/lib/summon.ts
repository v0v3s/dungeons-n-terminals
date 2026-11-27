// Summoning a companion: the creature (when the rules allow more than one)
// and its type (when they offer types), then it joins. A spell asks this
// when it is cast, at the level it was cast at; a feature from the
// Companions tab.
import type { Giver } from "./api";
import type { SheetCtx } from "./sheetctx";

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export function summon(ctx: SheetCtx, g: Giver, spell_level: number | null) {
  const withCreature = (creature: string) => {
    const add = (creature_type: string | null) =>
      ctx.act({ command: "add_companion", creature, from: g.id, spell_level, creature_type, name: "" });
    if (g.types.length) ctx.menu(`${g.kind}: which type?`, g.types.map((t) => [cap(t), () => add(t)]));
    else add(null);
  };
  if (g.creatures.length === 1) withCreature(g.creatures[0][0]);
  else ctx.pick({ type: "creatures", giver: g.id }, (values) => values[0] && withCreature(values[0]));
}
