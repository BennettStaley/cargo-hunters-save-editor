import { Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import {
  applyItem, broadcastChange, copyItem, currentSnapshot, deleteItems, loadState, moveItem,
  onStateChanged, openContainerWindow, pasteItem, repairItems,
  type ItemView, type Snapshot, type Source,
} from "../api";
import VaultGrid from "./VaultGrid";
import EditBar from "./EditBar";
import ContextMenu, { type MenuItem } from "./ContextMenu";

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
  const [selId, setSelId] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal("LOADING…");
  const [menu, setMenu] = createSignal<{ x: number; y: number; id: string } | null>(null);

  const refresh = async () => {
    try {
      setSnap(await currentSnapshot());
    } catch {
      try { setSnap(await loadState()); } catch (e) { setStatus(String(e)); }
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
    const id = selId();
    if (!id) return;
    try { const s = await deleteItems([id]); setSelId(null); commit(s, "DELETED · staged"); } catch (e) { setStatus(String(e)); }
  };
  // right-click copy / paste / delete; paste targets THIS container.
  const menuItems = (): MenuItem[] => {
    const m = menu(); const s = snap();
    if (!m || !s) return [];
    const it = children().find((x) => x.id === m.id);
    const items: MenuItem[] = [
      { label: it?.isContainer ? "Copy (with contents)" : "Copy",
        action: async () => { try { commit(await copyItem(p.source, m.id), "COPIED"); } catch (e) { setStatus(String(e)); } } },
      { label: s.clipboard ? `Paste "${s.clipboard}" here` : "Paste (clipboard empty)", disabled: !s.clipboard,
        action: async () => { try { commit(await pasteItem(p.source, p.ownerId), "PASTED · staged"); } catch (e) { setStatus(String(e)); } } },
    ];
    if (it?.isContainer && s.clipboard) {
      items.push({ label: `Paste "${s.clipboard}" into ${it.name}`,
        action: async () => { try { commit(await pasteItem(p.source, m.id), "PASTED · staged"); } catch (e) { setStatus(String(e)); } } });
    }
    items.push({ label: "Delete", danger: true,
      action: async () => { try { const r = await deleteItems([m.id]); if (selId() === m.id) setSelId(null); commit(r, "DELETED · staged"); } catch (e) { setStatus(String(e)); } } });
    return items;
  };

  return (
    <div class="app cwin">
      <div class="pane-head">
        <span class="h">{p.label.toUpperCase()}</span>
        <span class="sub">{children().length} ITEMS{cols() ? ` · ${cols()} WIDE` : ""}</span>
      </div>
      <div class="scroll cwin-scroll">
        <Show when={snap()} fallback={<div class="modal-empty">{status()}</div>}>
          <Show when={children().length} fallback={<div class="modal-empty">This container is empty.</div>}>
            <VaultGrid items={children()} cols={cols()} selectedId={selId()}
              onSelect={setSelId} onMove={onMoveItem} onActivate={onActivate}
              onContextMenu={(id, x, y) => setMenu({ id, x, y })} />
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
