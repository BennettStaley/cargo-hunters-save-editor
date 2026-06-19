import { For, Show, createMemo, createSignal } from "solid-js";
import type { ItemView } from "../api";
import { gridBlocks, type Block } from "../layout";
import ItemTile from "./ItemTile";

const CELL = 58;

interface Props {
  items: ItemView[]; // children of the container being shown
  selectedIds: string[];
  /** Select an item. `additive` (ctrl/cmd held) toggles it in the multi-selection. */
  onSelect: (id: string, additive: boolean) => void;
  /** Result of a rubber-band box selection (replaces the selection). */
  onSelectBox?: (ids: string[]) => void;
  onActivate?: (id: string) => void;
  /** Commit a drag-move to grid cell (i,j). If absent, dragging is disabled. */
  onMove?: (id: string, i: number, j: number) => void;
  /** Right-click: item id under the cursor, or null for empty space. */
  onContextMenu?: (id: string | null, x: number, y: number) => void;
  cols?: number;
}

interface Drag {
  id: string;
  block: Block;
  offI: number;
  offJ: number;
  ti: number;
  tj: number;
  ok: boolean;
  moved: boolean;
}

interface Box {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export default function VaultGrid(p: Props) {
  const layout = createMemo(() => gridBlocks(p.items));
  const cols = () => p.cols ?? layout().gw;
  const rows = () => Math.max(layout().gh, 1);
  const [drag, setDrag] = createSignal<Drag | null>(null);
  const [box, setBox] = createSignal<Box | null>(null);
  let gridEl: HTMLDivElement | undefined;

  const px = (clientX: number, clientY: number): [number, number] => {
    const r = gridEl!.getBoundingClientRect();
    return [clientX - r.left, clientY - r.top];
  };
  const cellAt = (clientX: number, clientY: number): [number, number] => {
    const [x, y] = px(clientX, clientY);
    return [Math.floor(x / CELL), Math.floor(y / CELL)];
  };
  const blockAt = (ci: number, cj: number): Block | undefined =>
    layout().blocks.find((x) => ci >= x.i && ci < x.i + x.w && cj >= x.j && cj < x.j + x.h);

  const fits = (b: Block, ti: number, tj: number): boolean => {
    if (ti < 0 || tj < 0 || ti + b.w > cols()) return false;
    const occ = new Set<string>();
    for (const o of layout().blocks) {
      if (o.item.id === b.item.id) continue;
      for (let di = 0; di < o.w; di++) for (let dj = 0; dj < o.h; dj++) occ.add(`${o.i + di},${o.j + dj}`);
    }
    for (let di = 0; di < b.w; di++) for (let dj = 0; dj < b.h; dj++) if (occ.has(`${ti + di},${tj + dj}`)) return false;
    return true;
  };

  let lastTap = { id: "", t: -1e9 };
  let pressItem: string | null = null; // item under a box-start press (for click-to-select)
  const onPointerDown = (e: PointerEvent) => {
    if (e.button !== 0) return; // left button only; right-click is handled by onContext
    const [x, y] = px(e.clientX, e.clientY);
    const ci = Math.floor(x / CELL);
    const cj = Math.floor(y / CELL);
    const b = blockAt(ci, cj);
    const additive = e.ctrlKey || e.metaKey;

    if (additive) {
      if (b) p.onSelect(b.item.id, true); // toggle in multi-selection
      return;
    }
    if (b) {
      // Double-click / arrow badge opens a container (pointer capture eats dblclick).
      const rightPx = (b.i + b.w) * CELL;
      const topPx = b.j * CELL;
      const onBadge = b.item.isContainer && x >= rightPx - 22 && x <= rightPx && y >= topPx && y <= topPx + 22;
      const doubleClick = b.item.id === lastTap.id && e.timeStamp - lastTap.t < 350;
      if (onBadge || doubleClick) {
        lastTap = { id: "", t: -1e9 };
        p.onSelect(b.item.id, false);
        p.onActivate?.(b.item.id);
        return;
      }
      lastTap = { id: b.item.id, t: e.timeStamp };
      // Move only when dragging the item that is the sole current selection;
      // otherwise a drag (even starting on an item) is a box-select.
      if (p.onMove && p.selectedIds.length === 1 && p.selectedIds[0] === b.item.id) {
        gridEl!.setPointerCapture(e.pointerId);
        setDrag({ id: b.item.id, block: b, offI: ci - b.i, offJ: cj - b.j, ti: b.i, tj: b.j, ok: true, moved: false });
        return;
      }
    }
    // Box-select from anywhere (over items or empty). A no-drag release just
    // selects the pressed item (or clears, on empty space).
    gridEl!.setPointerCapture(e.pointerId);
    pressItem = b?.item.id ?? null;
    setBox({ x0: x, y0: y, x1: x, y1: y });
  };

  const onPointerMove = (e: PointerEvent) => {
    if (box()) {
      const [x, y] = px(e.clientX, e.clientY);
      setBox({ ...box()!, x1: x, y1: y });
      return;
    }
    const d = drag();
    if (!d) return;
    const [ci, cj] = cellAt(e.clientX, e.clientY);
    const ti = ci - d.offI;
    const tj = cj - d.offJ;
    const moved = d.moved || ti !== d.block.i || tj !== d.block.j;
    setDrag({ ...d, ti, tj, ok: fits(d.block, ti, tj), moved });
  };

  const onPointerUp = () => {
    const bx = box();
    if (bx) {
      setBox(null);
      const x0 = Math.min(bx.x0, bx.x1), x1 = Math.max(bx.x0, bx.x1);
      const y0 = Math.min(bx.y0, bx.y1), y1 = Math.max(bx.y0, bx.y1);
      const item = pressItem;
      pressItem = null;
      if (x1 - x0 < 5 && y1 - y0 < 5) {
        // A plain click: select the pressed item, or clear on empty space.
        if (item) p.onSelect(item, false);
        else p.onSelectBox?.([]);
        return;
      }
      const hit = layout().blocks.filter((b) => {
        const bx0 = b.i * CELL, bx1 = (b.i + b.w) * CELL, by0 = b.j * CELL, by1 = (b.j + b.h) * CELL;
        return bx0 < x1 && bx1 > x0 && by0 < y1 && by1 > y0;
      });
      p.onSelectBox?.(hit.map((b) => b.item.id));
      return;
    }
    const d = drag();
    setDrag(null);
    if (d && d.moved && d.ok && (d.ti !== d.block.i || d.tj !== d.block.j)) {
      p.onMove?.(d.id, d.ti, d.tj);
    }
  };

  // An interrupted pointer (OS gesture, touch cancel, lost capture) must reset
  // the in-progress drag/box state WITHOUT committing a move or selection,
  // otherwise the grid gets stuck mid-drag.
  const onPointerCancel = () => {
    setBox(null);
    setDrag(null);
    pressItem = null;
  };

  const onContext = (e: MouseEvent) => {
    e.preventDefault();
    const [x, y] = px(e.clientX, e.clientY);
    const b = blockAt(Math.floor(x / CELL), Math.floor(y / CELL));
    // Right-clicking an item NOT already in the selection selects just it;
    // right-clicking a selected item keeps the (multi-)selection intact.
    if (b && !p.selectedIds.includes(b.item.id)) p.onSelect(b.item.id, false);
    p.onContextMenu?.(b?.item.id ?? null, e.clientX, e.clientY);
  };

  return (
    <div
      class="grid"
      ref={gridEl}
      style={{ width: `${cols() * CELL}px`, height: `${rows() * CELL}px` }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerCancel}
      onLostPointerCapture={onPointerCancel}
      onContextMenu={onContext}
    >
      <For each={layout().blocks}>
        {(b) => (
          <ItemTile
            item={b.item}
            left={b.i * CELL}
            top={b.j * CELL}
            width={b.w * CELL}
            height={b.h * CELL}
            selected={p.selectedIds.includes(b.item.id)}
          />
        )}
      </For>
      <Show when={drag()?.moved}>
        <div class="drag-ghost" classList={{ bad: !drag()!.ok }}
          style={{ left: `${drag()!.ti * CELL}px`, top: `${drag()!.tj * CELL}px`,
            width: `${drag()!.block.w * CELL}px`, height: `${drag()!.block.h * CELL}px` }} />
      </Show>
      <Show when={box()}>
        <div class="select-box" style={{
          left: `${Math.min(box()!.x0, box()!.x1)}px`, top: `${Math.min(box()!.y0, box()!.y1)}px`,
          width: `${Math.abs(box()!.x1 - box()!.x0)}px`, height: `${Math.abs(box()!.y1 - box()!.y0)}px` }} />
      </Show>
    </div>
  );
}
