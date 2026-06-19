import { Show } from "solid-js";
import type { ItemView } from "../api";
import { resolveIcon, iconUrl } from "../icons";
import { conditionFrac } from "../layout";

interface Props {
  item: ItemView;
  left: number;
  top: number;
  width: number;
  height: number;
  selected: boolean;
  // All interaction handlers are optional: inside the grid the parent owns
  // pointer/context handling and passes none (the tile is a pure visual).
  onSelect?: (id: string) => void;
  onActivate?: (id: string) => void;
  onContextMenu?: (id: string, x: number, y: number) => void;
}

function barColor(frac: number): string {
  if (frac > 0.5) return "var(--good)";
  if (frac > 0.2) return "var(--amber)";
  return "var(--danger)";
}

export default function ItemTile(p: Props) {
  const frac = () => conditionFrac(p.item);
  // Prefer the real rendered model image; fall back to the category icon, then a
  // generic icon. (Rendered icons live in /sprites/items/<templateId>.png.)
  const sources = [
    `/sprites/items/${p.item.templateId}.webp`,
    iconUrl(resolveIcon(p.item.visualName, p.item.name)),
    iconUrl("Icon_Surplus"),
  ];
  let step = 0;
  return (
    <div
      class="tile"
      classList={{ sel: p.selected }}
      style={{ left: `${p.left}px`, top: `${p.top}px`, width: `${p.width}px`, height: `${p.height}px` }}
      onClick={p.onSelect ? () => p.onSelect!(p.item.id) : undefined}
      onDblClick={p.onActivate ? () => p.onActivate!(p.item.id) : undefined}
      onContextMenu={p.onContextMenu
        ? (e) => { e.preventDefault(); p.onSelect?.(p.item.id); p.onContextMenu!(p.item.id, e.clientX, e.clientY); }
        : undefined}
      title={p.item.name}
    >
      <img class="ico" src={sources[0]} draggable={false}
        onError={(e) => { step += 1; if (step < sources.length) (e.currentTarget as HTMLImageElement).src = sources[step]; }} />
      <div class="nm">{p.item.name}</div>
      <Show when={p.item.qty !== null}>
        <div class="qty">{p.item.qty}</div>
      </Show>
      <Show when={frac() !== null}>
        <div class="bar"><span style={{ width: `${frac()! * 100}%`, background: barColor(frac()!) }} /></div>
      </Show>
      <Show when={p.item.isContainer}>
        <div class="badge">›</div>
      </Show>
    </div>
  );
}
