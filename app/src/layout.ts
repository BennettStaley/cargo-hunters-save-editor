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

/** Lay out the children of one container. Footprints (`gridW`/`gridH`) are
 * computed by the engine — including assembled-weapon sizing recovered from the
 * packing — so display matches occupancy exactly. */
export function gridBlocks(items: ItemView[]): { blocks: Block[]; gw: number; gh: number } {
  const blocks: Block[] = items
    .filter((it) => it.i !== null && it.j !== null && it.i! >= 0 && it.j! >= 0)
    .map((it) => ({ item: it, i: it.i!, j: it.j!, w: Math.max(it.gridW, 1), h: Math.max(it.gridH, 1) }));
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
