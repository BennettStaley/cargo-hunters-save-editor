import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import {
  applyItem, broadcastChange, copyItem, currentSnapshot, deleteItems, moveItem,
  onStateChanged, openContainerWindow, pasteItem, repairItems,
  type ItemView, type Snapshot, type Source,
} from "../api";
import VaultGrid from "./VaultGrid";
import EditBar from "./EditBar";
import ContextMenu, { type MenuItem } from "./ContextMenu";
import { resolveIcon, iconUrl } from "../icons";
import { conditionFrac } from "../layout";

// A weapon isn't storage: its children are attachments (barrel, mag, receiver…),
// not free-placement grid cargo. We detect it by VisualName and render a slot
// list instead of the grid pop-out (which tiled the wide part-renders so they
// overlapped). Nested items carry no `slot`, so VisualName is the only signal.
const isWeaponVisual = (v: string): boolean => /(^|\/)Weapons\//i.test(v);

// Same fallback chain as ItemTile: real render -> category icon -> generic.
const imgSources = (it: ItemView): string[] => [
  `/sprites/items/${it.templateId}.webp`,
  iconUrl(resolveIcon(it.visualName, it.name)),
  iconUrl("Icon_Surplus"),
];

// Canonical part type from the prefab name: drop the weapon-family token and any
// version suffix, humanize camelCase. "Oscar_AmmoCase" -> "Ammo Case".
const partLabel = (visualName: string): string => {
  const base = visualName.split("/").pop()?.replace(/\.prefab$/i, "") ?? "";
  const toks = base.split("_").filter((t) => t && !/^\d+$/.test(t) && t.length > 1);
  const rest = toks.length > 1 ? toks.slice(1) : toks;
  return rest.join(" ").replace(/([a-z])([A-Z])/g, "$1 $2") || "Part";
};

const barColor = (f: number): string => (f > 0.5 ? "var(--good)" : f > 0.2 ? "var(--amber)" : "var(--danger)");

interface Props {
  source: Source;
  ownerId: string;
  label: string;
}

/** Self-contained view of one container, rendered in its own OS window. The
 * Rust backend state is shared across all windows, so edits made here show up
 * in the main window (and vice-versa) via the cross-window change event. */
export default function ContainerWindow(p: Props) {
  const [snap, setSnap] = createSignal<Snapshot | null>(null);
  const [selIds, setSelIds] = createSignal<string[]>([]);
  const selId = () => selIds()[selIds().length - 1] ?? null;
  const setSel = (id: string | null) => setSelIds(id ? [id] : []);
  const toggleSel = (id: string) =>
    setSelIds((cur) => (cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id]));
  const [status, setStatus] = createSignal("LOADING…");
  const [menu, setMenu] = createSignal<{ x: number; y: number; id: string | null } | null>(null);

  const refresh = async () => {
    // Keep the last good snapshot on error. NEVER fall back to loadState() here:
    // it re-reads the shared session from disk and clears dirty, silently
    // discarding staged edits made in every window.
    try {
      setSnap(await currentSnapshot());
    } catch (e) {
      setStatus(String(e));
    }
  };
  onMount(async () => {
    await refresh();
    setStatus("READY");
    const un = await onStateChanged(refresh);
    onCleanup(() => un());
  });

  const commit = (s: Snapshot, msg: string) => { setSnap(s); setStatus(msg); void broadcastChange(); };

  const children = createMemo<ItemView[]>(() => {
    const s = snap();
    if (!s) return [];
    const arr = p.source === "inventory" ? s.inventory : p.source === "equipment" ? s.equipment : s.shelter;
    return arr.filter((it) => it.parentId === p.ownerId);
  });
  const cols = createMemo<number | undefined>(() => {
    const c = snap()?.containers.find((c) => c.source === p.source && c.ownerItemId === p.ownerId);
    return c?.gridWidth ?? undefined;
  });
  // The container's owner item, to tell a weapon (attachment list) from real
  // storage (grid). The owner lives in the same source array as its children.
  const owner = createMemo<ItemView | null>(() => {
    const s = snap();
    if (!s) return null;
    const arr = p.source === "inventory" ? s.inventory : p.source === "equipment" ? s.equipment : s.shelter;
    return arr.find((it) => it.id === p.ownerId) ?? null;
  });
  const isWeapon = createMemo(() => isWeaponVisual(owner()?.visualName ?? ""));
  const selected = createMemo(() => children().find((it) => it.id === selId()) ?? null);
  const category = (it: ItemView | null) => (it ? it.visualName.split("/")[0] || "Item" : "");

  const onMoveItem = async (id: string, i: number, j: number) => {
    try { commit(await moveItem(p.source, id, i, j), `MOVED → (${i},${j}) · staged`); } catch (e) { setStatus(String(e)); }
  };
  const onActivate = (id: string) => {
    const it = children().find((x) => x.id === id);
    if (it?.isContainer) void openContainerWindow(p.source, it.id, it.name);
  };
  const onApply = async (v: { qty: number | null; condition: number | null; durability: number | null }) => {
    const id = selId();
    if (!id) return;
    try { commit(await applyItem(p.source, id, v.qty, v.condition, v.durability), "EDITED · staged"); } catch (e) { setStatus(String(e)); }
  };
  const onRepair = async () => {
    const id = selId();
    if (!id) return;
    try { commit(await repairItems([id]), "REPAIRED · staged"); } catch (e) { setStatus(String(e)); }
  };
  const onDelete = async () => {
    const ids = selIds();
    if (!ids.length) return;
    try { const s = await deleteItems(ids); setSelIds([]); commit(s, `DELETED ${ids.length} · staged`); } catch (e) { setStatus(String(e)); }
  };
  // right-click copy / paste / delete; paste targets THIS container. Bulk on multi-select.
  const pasteHere = (s: Snapshot): MenuItem => ({
    label: s.clipboard ? `Paste "${s.clipboard}" here` : "Paste (clipboard empty)",
    disabled: !s.clipboard,
    action: async () => { try { commit(await pasteItem(p.source, p.ownerId), "PASTED · staged"); } catch (e) { setStatus(String(e)); } },
  });
  const menuItems = (): MenuItem[] => {
    const m = menu(); const s = snap();
    if (!m || !s) return [];
    if (m.id === null) return [pasteHere(s)]; // right-click empty space -> paste
    const id = m.id;
    // Only count items still present in this container (a cross-window delete
    // can leave stale ids in selIds; the label must reflect reality).
    const live = new Set(children().map((c) => c.id));
    const sel = selIds().filter((x) => live.has(x));
    if (sel.length > 1 && sel.includes(id)) {
      return [
        { label: `Repair ${sel.length} items`,
          action: async () => { try { commit(await repairItems(sel), `REPAIRED ${sel.length} · staged`); } catch (e) { setStatus(String(e)); } } },
        { label: `Delete ${sel.length} items`, danger: true,
          action: async () => { try { const r = await deleteItems(sel); setSelIds([]); commit(r, `DELETED ${sel.length} · staged`); } catch (e) { setStatus(String(e)); } } },
      ];
    }
    const it = children().find((x) => x.id === id);
    const items: MenuItem[] = [
      { label: it?.isContainer ? "Copy (with contents)" : "Copy",
        action: async () => { try { commit(await copyItem(p.source, id), "COPIED"); } catch (e) { setStatus(String(e)); } } },
      pasteHere(s),
    ];
    if (it?.isContainer && s.clipboard) {
      items.push({ label: `Paste "${s.clipboard}" into ${it.name}`,
        action: async () => { try { commit(await pasteItem(p.source, id), "PASTED · staged"); } catch (e) { setStatus(String(e)); } } });
    }
    items.push({ label: "Delete", danger: true,
      action: async () => { try { const r = await deleteItems([id]); setSelIds([]); commit(r, "DELETED · staged"); } catch (e) { setStatus(String(e)); } } });
    return items;
  };

  return (
    <div class="app cwin">
      <div class="pane-head">
        <span class="h">{p.label.toUpperCase()}</span>
        <span class="sub">
          {children().length} {isWeapon() ? "ATTACHMENTS" : `ITEMS${cols() ? ` · ${cols()} WIDE` : ""}`}
        </span>
      </div>
      <div class="scroll cwin-scroll">
        <Show when={snap()} fallback={<div class="modal-empty">{status()}</div>}>
          <Show when={children().length}
            fallback={<div class="modal-empty">{isWeapon() ? "No attachments." : "This container is empty."}</div>}>
            <Show when={isWeapon()} fallback={
              <VaultGrid items={children()} cols={cols()} selectedIds={selIds()}
                onSelect={(id, additive) => (additive ? toggleSel(id) : setSel(id))}
                onSelectBox={(ids) => setSelIds(ids)}
                onMove={onMoveItem} onActivate={onActivate}
                onContextMenu={(id, x, y) => setMenu({ id, x, y })} />
            }>
              <div class="attach-list">
                <For each={children()}>
                  {(it) => {
                    const srcs = imgSources(it);
                    let step = 0;
                    const frac = () => conditionFrac(it);
                    return (
                      <div class="attach-row" classList={{ sel: selIds().includes(it.id) }}
                        onClick={(e) => (e.ctrlKey ? toggleSel(it.id) : setSel(it.id))}
                        onDblClick={() => onActivate(it.id)}
                        onContextMenu={(e) => { e.preventDefault(); setSel(it.id); setMenu({ id: it.id, x: e.clientX, y: e.clientY }); }}>
                        <img class="attach-ico" src={srcs[0]} draggable={false}
                          onError={(e) => { step += 1; if (step < srcs.length) (e.currentTarget as HTMLImageElement).src = srcs[step]; }} />
                        <div class="attach-text">
                          <div class="attach-name">{it.name}</div>
                          <div class="attach-slot">{partLabel(it.visualName)}</div>
                        </div>
                        <Show when={frac() !== null}>
                          <div class="attach-bar"><span style={{ width: `${frac()! * 100}%`, background: barColor(frac()!) }} /></div>
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </div>
            </Show>
          </Show>
        </Show>
      </div>
      <EditBar item={selected()} category={category(selected())}
        onApply={onApply} onRepair={onRepair} onDelete={onDelete} />
      <div class="statusbar">{status()}</div>
      <Show when={menu()}>
        {(m) => <ContextMenu x={m().x} y={m().y} items={menuItems()} onClose={() => setMenu(null)} />}
      </Show>
    </div>
  );
}
