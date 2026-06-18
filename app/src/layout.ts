// Grid placement helpers. The catalog gives each item a BASE footprint, but
// ASSEMBLED WEAPONS (those carrying a BaseComponent part-grid) render larger:
// the game packs attachments so the true footprint is the gap to the next item
// in the row (width) / column (height). Ported from the Python `_blocks_for`.

import type { ItemView } from "./api";

export interface Block {
  item: ItemView;
  i: number;
  j: number;
  w: number;
  h: number;
}

/** An item's true footprint: catalog base size, expanded to the assembled
 * part-grid (BaseComponent_width/_height) for weapons. This must match the
 * engine's `occupied_size` so what you see is exactly what's reserved. */
function footprint(it: ItemView): { w: number; h: number } {
  return {
    w: Math.max(it.baseW, it.asmW ?? 0, 1),
    h: Math.max(it.baseH, it.asmH ?? 0, 1),
  };
}

/** Lay out the children of one container (items already filtered to a parent). */
export function gridBlocks(items: ItemView[]): { blocks: Block[]; gw: number; gh: number } {
  const blocks: Block[] = items
    .filter((it) => it.i !== null && it.j !== null && it.i! >= 0 && it.j! >= 0)
    .map((it) => {
      const { w, h } = footprint(it);
      return { item: it, i: it.i!, j: it.j!, w, h };
    });
  const gw = blocks.reduce((m, b) => Math.max(m, b.i + b.w), 0) || 1;
  const gh = blocks.reduce((m, b) => Math.max(m, b.j + b.h), 0) || 1;
  return { blocks, gw, gh };
}

/** Condition/durability as a 0..1 fraction for the bar, or null if no stat. */
export function conditionFrac(it: ItemView): number | null {
  if (it.conditionD !== null) return Math.max(0, Math.min(1, it.conditionD / 4.0));
  if (it.durability !== null && it.durabilityMd) {
    return Math.max(0, Math.min(1, it.durability / it.durabilityMd));
  }
  return null;
}
