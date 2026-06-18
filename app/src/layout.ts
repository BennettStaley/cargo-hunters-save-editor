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

/** Lay out the children of one container (items already filtered to a parent). */
export function gridBlocks(items: ItemView[]): { blocks: Block[]; gw: number; gh: number } {
  const placed = items
    .filter((it) => it.i !== null && it.j !== null && it.i! >= 0 && it.j! >= 0)
    .map((it) => ({ it, i: it.i!, j: it.j!, w0: it.baseW, h0: it.baseH, w: it.baseW, h: it.baseH }));

  for (const p of placed) {
    const assembled = p.it.asmW !== null || p.it.asmH !== null;
    if (!assembled) continue;
    const right = placed.filter((q) => q !== p && q.j === p.j && q.i > p.i).map((q) => q.i);
    const below = placed.filter((q) => q !== p && q.i === p.i && q.j > p.j).map((q) => q.j);
    p.w = Math.max(p.w0, right.length ? Math.min(...right) - p.i : p.w0);
    p.h = Math.max(p.h0, below.length ? Math.min(...below) - p.j : p.h0);
  }

  const blocks: Block[] = placed.map((p) => ({ item: p.it, i: p.i, j: p.j, w: p.w, h: p.h }));
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
