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

  const onPointerDown = (e: PointerEvent) => {
    const [ci, cj] = cellAt(e.clientX, e.clientY);
    const b = layout().blocks.find((x) => ci >= x.i && ci < x.i + x.w && cj >= x.j && cj < x.j + x.h);
    if (!b) return;
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

  return (
    <div
      class="grid"
      ref={gridEl}
      style={{ width: `${cols() * CELL}px`, height: `${rows() * CELL}px` }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
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
