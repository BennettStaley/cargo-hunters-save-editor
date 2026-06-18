import { Show, createEffect, createSignal } from "solid-js";
import type { ItemView } from "../api";

interface Props {
  item: ItemView | null;
  category: string;
  onApply: (vals: { qty: number | null; condition: number | null; durability: number | null }) => void;
  onRepair: () => void;
  onDelete: () => void;
}

export default function EditBar(p: Props) {
  const [qty, setQty] = createSignal("");
  const [cond, setCond] = createSignal("");
  const [dur, setDur] = createSignal("");

  // Reseed the fields whenever the selected item changes.
  createEffect(() => {
    const it = p.item;
    setQty(it?.qty != null ? String(it.qty) : "");
    setCond(it?.conditionD != null ? String(it.conditionD) : "");
    setDur(it?.durability != null ? String(it.durability) : "");
  });

  const num = (s: string): number | null => {
    const t = s.trim();
    if (t === "") return null;
    const v = Number(t);
    return Number.isFinite(v) ? v : null;
  };

  return (
    <div class="editbar">
      <span class="sel-name">{p.item ? p.item.name.toUpperCase() : "- SELECT AN ITEM -"}</span>
      <Show when={p.item}>
        <span class="sel-sub">
          {p.category} · {p.item!.baseW}×{p.item!.baseH}
        </span>
      </Show>
      <div class="grow" />
      {/* Fields are editable for any selected item; a value can be SET even if
          the item doesn't have that stat yet (the engine adds it on apply).
          Leave a field blank to leave that stat untouched. */}
      <label class="fld">
        QTY
        <input type="number" min="0" value={qty()} disabled={!p.item} placeholder="-"
          onInput={(e) => setQty(e.currentTarget.value)} />
      </label>
      <label class="fld">
        COND
        <input class="narrow" type="number" step="0.5" min="0" max="4" value={cond()}
          disabled={!p.item} placeholder="-" onInput={(e) => setCond(e.currentTarget.value)} />
      </label>
      <label class="fld">
        DUR
        <input type="number" step="1" min="0" value={dur()}
          disabled={!p.item} placeholder="-" onInput={(e) => setDur(e.currentTarget.value)} />
      </label>
      <button class="primary" disabled={!p.item}
        onClick={() => p.onApply({ qty: num(qty()), condition: num(cond()), durability: num(dur()) })}>
        APPLY
      </button>
      <button disabled={!p.item} onClick={() => p.onRepair()}>REPAIR</button>
      <button class="danger" disabled={!p.item} onClick={() => p.onDelete()}>DELETE</button>
    </div>
  );
}
