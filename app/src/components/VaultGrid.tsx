import { For, Show, createMemo, createSignal } from "solid-js";
import type { ItemView } from "../api";
import { gridBlocks, type Block } from "../layout";
import ItemTile from "./ItemTile";

const CELL = 58;

interface Props {
  items: ItemView[]; // children of the container being shown
  selectedId: string | null;
  onSelect: (id: string) => void;
  onActivate?: (id: string) => void;
  /** Commit a drag-move to grid cell (i,j). If absent, dragging is disabled. */
  onMove?: (id: string, i: number, j: number) => void;
  /** Right-click an item: (id, screenX, screenY). */
  onContextMenu?: (id: string, x: number, y: number) => void;
  cols?: number;
}

interface Drag {
  id: string;
  block: Block;
  offI: number; // pointer offset within the block, in cells
  offJ: number;
  ti: number;
  tj: number;
  ok: boolean;
  moved: boolean;
}

export default function VaultGrid(p: Props) {
  const layout = createMemo(() => gridBlocks(p.items));
  const cols = () => p.cols ?? layout().gw;
  const rows = () => Math.max(layout().gh, 1);
  const [drag, setDrag] = createSignal<Drag | null>(null);
  let gridEl: HTMLDivElement | undefined;

  const cellAt = (clientX: number, clientY: number): [number, number] => {
    const r = gridEl!.getBoundingClientRect();
    return [Math.floor((clientX - r.left) / CELL), Math.floor((clientY - r.top) / CELL)];
  };

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

  // Double-click is detected here (not via ItemTile.onDblClick): the grid uses
  // pointer capture for dragging, which retargets native click/dblclick away
  // from the tiles. A click on the container's ▸ badge also opens it.
  let lastTap = { id: "", t: -1e9 };
  const onPointerDown = (e: PointerEvent) => {
    const r = gridEl!.getBoundingClientRect();
    const px = e.clientX - r.left;
    const py = e.clientY - r.top;
    const ci = Math.floor(px / CELL);
    const cj = Math.floor(py / CELL);
    const b = layout().blocks.find((x) => ci >= x.i && ci < x.i + x.w && cj >= x.j && cj < x.j + x.h);
    if (!b) return;
    const rightPx = (b.i + b.w) * CELL;
    const topPx = b.j * CELL;
    const onBadge = b.item.isContainer && px >= rightPx - 22 && px <= rightPx && py >= topPx && py <= topPx + 22;
    const doubleClick = b.item.id === lastTap.id && e.timeStamp - lastTap.t < 350;
    if (onBadge || doubleClick) {
      lastTap = { id: "", t: -1e9 };
      p.onSelect(b.item.id);
      p.onActivate?.(b.item.id);
      return; // don't begin a drag
    }
    lastTap = { id: b.item.id, t: e.timeStamp };
    p.onSelect(b.item.id);
    if (!p.onMove) return;
    gridEl!.setPointerCapture(e.pointerId);
    setDrag({ id: b.item.id, block: b, offI: ci - b.i, offJ: cj - b.j, ti: b.i, tj: b.j, ok: true, moved: false });
  };

  const onPointerMove = (e: PointerEvent) => {
    const d = drag();
    if (!d) return;
    const [ci, cj] = cellAt(e.clientX, e.clientY);
    const ti = ci - d.offI;
    const tj = cj - d.offJ;
    const moved = d.moved || ti !== d.block.i || tj !== d.block.j;
    setDrag({ ...d, ti, tj, ok: fits(d.block, ti, tj), moved });
  };

  const onPointerUp = () => {
    const d = drag();
    setDrag(null);
    if (d && d.moved && d.ok && (d.ti !== d.block.i || d.tj !== d.block.j)) {
      p.onMove?.(d.id, d.ti, d.tj);
    }
  };

  const onContext = (e: MouseEvent) => {
    e.preventDefault();
    const r = gridEl!.getBoundingClientRect();
    const ci = Math.floor((e.clientX - r.left) / CELL);
    const cj = Math.floor((e.clientY - r.top) / CELL);
    const b = layout().blocks.find((x) => ci >= x.i && ci < x.i + x.w && cj >= x.j && cj < x.j + x.h);
    if (b) {
      p.onSelect(b.item.id);
      p.onContextMenu?.(b.item.id, e.clientX, e.clientY);
    }
  };

  return (
    <div
      class="grid"
      ref={gridEl}
      style={{ width: `${cols() * CELL}px`, height: `${rows() * CELL}px` }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
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
            selected={p.selectedId === b.item.id}
            onSelect={p.onSelect}
            onActivate={p.onActivate}
          />
        )}
      </For>
      <Show when={drag()?.moved}>
        <div
          class="drag-ghost"
          classList={{ bad: !drag()!.ok }}
          style={{
            left: `${drag()!.ti * CELL}px`,
            top: `${drag()!.tj * CELL}px`,
            width: `${drag()!.block.w * CELL}px`,
            height: `${drag()!.block.h * CELL}px`,
          }}
        />
      </Show>
    </div>
  );
}
