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
  onSelect: (id: string) => void;
  onActivate?: (id: string) => void;
}

function barColor(frac: number): string {
  if (frac > 0.5) return "var(--good)";
  if (frac > 0.2) return "var(--amber)";
  return "var(--danger)";
}

export default function ItemTile(p: Props) {
  const frac = () => conditionFrac(p.item);
  return (
    <div
      class="tile"
      classList={{ sel: p.selected }}
      style={{ left: `${p.left}px`, top: `${p.top}px`, width: `${p.width}px`, height: `${p.height}px` }}
      onClick={() => p.onSelect(p.item.id)}
      onDblClick={() => p.onActivate?.(p.item.id)}
      title={p.item.name}
    >
      <img class="ico" src={iconUrl(resolveIcon(p.item.visualName, p.item.name))} draggable={false}
        onError={(e) => ((e.currentTarget as HTMLImageElement).src = iconUrl("Icon_Surplus"))} />
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
