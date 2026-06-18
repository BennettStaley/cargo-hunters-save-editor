import { For, Show, createMemo, createSignal } from "solid-js";
import type { CatalogEntry, Container } from "../api";
import { resolveIcon, iconUrl } from "../icons";
import "../catalog.css";

interface Props {
  catalog: CatalogEntry[]; // ~1595 entries, already sorted by name
  containers: Container[]; // possible destinations (Backpack, equipment, etc.)
  onAdd: (a: {
    templateId: string;
    source: string;
    ownerId: string;
    quantity: number | null;
    count: number;
    condition: number | null;
    durability: number | null;
    gridWidth: number;
  }) => void;
  onClose: () => void; // close/back button
}

const RESULT_CAP = 300;

function categoryOf(e: CatalogEntry): string {
  const top = (e.visualName || "").split("/")[0];
  return top ? top : "Other";
}

function numOrNull(s: string): number | null {
  const t = s.trim();
  if (t === "") return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
}

export default function CatalogBrowser(p: Props) {
  const [search, setSearch] = createSignal("");
  const [cat, setCat] = createSignal("ALL");
  const [selId, setSelId] = createSignal<string | null>(null);

  // Add-panel inputs.
  const [qty, setQty] = createSignal("");
  const [count, setCount] = createSignal("1");
  const [cond, setCond] = createSignal("");
  const [dur, setDur] = createSignal("");
  const defaultDest = () => {
    const bp = p.containers.find((c) => c.label === "Backpack");
    return (bp ?? p.containers[0])?.ownerItemId ?? "";
  };
  const [dest, setDest] = createSignal<string>("");

  const destId = () => dest() || defaultDest();
  const destContainer = () => p.containers.find((c) => c.ownerItemId === destId()) ?? null;

  const categories = createMemo(() => {
    const set = new Set<string>();
    for (const e of p.catalog) set.add(categoryOf(e));
    return ["ALL", ...Array.from(set).sort()];
  });

  const filtered = createMemo(() => {
    const q = search().trim().toLowerCase();
    const c = cat();
    return p.catalog.filter((e) => {
      if (c !== "ALL" && categoryOf(e) !== c) return false;
      if (q && !e.name.toLowerCase().includes(q)) return false;
      return true;
    });
  });

  const shown = createMemo(() => filtered().slice(0, RESULT_CAP));
  const selected = createMemo(() => p.catalog.find((e) => e.templateId === selId()) ?? null);

  const onIconError = (e: Event) => {
    (e.currentTarget as HTMLImageElement).src = iconUrl("Icon_Surplus");
  };

  const submit = () => {
    const e = selected();
    const c = destContainer();
    if (!e || !c) return;
    p.onAdd({
      templateId: e.templateId,
      source: c.source,
      ownerId: c.ownerItemId,
      quantity: numOrNull(qty()),
      count: Math.max(1, Math.floor(numOrNull(count()) ?? 1)),
      condition: numOrNull(cond()),
      durability: numOrNull(dur()),
      gridWidth: c.gridWidth ?? 10,
    });
  };

  return (
    <div class="catalog">
      <div class="catalog-head">
        <span class="h">ADD ITEMS</span>
        <input
          class="catalog-search"
          type="text"
          placeholder="Search items by name…"
          value={search()}
          onInput={(ev) => setSearch(ev.currentTarget.value)}
        />
        <span class="grow" />
        <button onClick={() => p.onClose()}>← BACK</button>
      </div>

      <div class="catalog-cats">
        <For each={categories()}>
          {(c) => (
            <span class="cat-chip" classList={{ active: cat() === c }} onClick={() => setCat(c)}>
              {c}
            </span>
          )}
        </For>
      </div>

      <div class="catalog-body">
        <div class="catalog-results">
          <For each={shown()}>
            {(e) => (
              <div
                class="cat-row"
                classList={{ sel: selId() === e.templateId }}
                onClick={() => setSelId(e.templateId)}
                title={e.name}
              >
                <img
                  class="thumb"
                  src={iconUrl(resolveIcon(e.visualName, e.name))}
                  onError={onIconError}
                  draggable={false}
                />
                <div class="meta">
                  <span class="nm">{e.name}</span>
                  <span class="sz">
                    {e.w}×{e.h}
                    {e.stackCapacity ? ` · ⌃${e.stackCapacity}` : ""}
                  </span>
                </div>
              </div>
            )}
          </For>
        </div>

        <div class="catalog-add">
          <Show
            when={selected()}
            fallback={<div class="empty">Select an item to add.</div>}
          >
            <img
              class="add-icon"
              src={iconUrl(resolveIcon(selected()!.visualName, selected()!.name))}
              onError={onIconError}
              draggable={false}
            />
            <div class="add-name">{selected()!.name}</div>
            <div class="add-sub">
              {categoryOf(selected()!)} · {selected()!.w}×{selected()!.h}
            </div>

            <label>
              Destination
              <select value={destId()} onChange={(ev) => setDest(ev.currentTarget.value)}>
                <For each={p.containers}>
                  {(c) => <option value={c.ownerItemId}>{c.label}</option>}
                </For>
              </select>
            </label>

            <label>
              Quantity (stack)
              <input
                type="number"
                min="0"
                value={qty()}
                placeholder="-"
                onInput={(ev) => setQty(ev.currentTarget.value)}
              />
              <Show when={selected()!.stackCapacity}>
                <span class="hint">stack capacity: {selected()!.stackCapacity}</span>
              </Show>
            </label>

            <label>
              Count (separate items)
              <input
                type="number"
                min="1"
                value={count()}
                onInput={(ev) => setCount(ev.currentTarget.value)}
              />
            </label>

            <label>
              Condition (0–4)
              <input
                type="number"
                min="0"
                max="4"
                step="0.5"
                value={cond()}
                placeholder="-"
                onInput={(ev) => setCond(ev.currentTarget.value)}
              />
            </label>

            <label>
              Durability
              <input
                type="number"
                min="0"
                value={dur()}
                placeholder="-"
                onInput={(ev) => setDur(ev.currentTarget.value)}
              />
            </label>

            <button class="primary" onClick={submit}>
              ADD TO {destContainer()?.label ?? "…"}
            </button>
          </Show>
        </div>
      </div>

      <div class="catalog-foot">
        showing {shown().length} of {filtered().length}
        {filtered().length > RESULT_CAP ? ` (capped at ${RESULT_CAP} - refine search)` : ""} ·{" "}
        {p.catalog.length} items in catalog
      </div>
    </div>
  );
}
