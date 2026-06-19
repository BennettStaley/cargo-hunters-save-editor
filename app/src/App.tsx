import { For, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import {
  addItems, applyItem, broadcastChange, copyItem, currentSnapshot, deleteItems, listCatalog, loadState,
  moveItem, moveToPage, onStateChanged, openContainerWindow, pasteItem, reloadFromDisk, repairItems, saveGame,
  setAccount, setSkill, topUpStacks,
  type CatalogEntry, type ItemView, type Snapshot, type Source,
} from "./api";
import Paperdoll from "./components/Paperdoll";
import VaultGrid from "./components/VaultGrid";
import EditBar from "./components/EditBar";
import CatalogBrowser from "./components/CatalogBrowser";
import CharacterPanel from "./components/CharacterPanel";
import ContextMenu, { type MenuItem } from "./components/ContextMenu";

type View = "inventory" | "add" | "character";

const ROMAN = ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"];
const roman = (n: number): string => ROMAN[n] ?? String(n);

export default function App() {
  const [snap, setSnap] = createSignal<Snapshot | null>(null);
  const [selIds, setSelIds] = createSignal<string[]>([]);
  const selId = () => selIds()[selIds().length - 1] ?? null; // primary = last selected
  const setSel = (id: string | null) => setSelIds(id ? [id] : []);
  const toggleSel = (id: string) =>
    setSelIds((cur) => (cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id]));
  const [status, setStatus] = createSignal("LOADING…");
  const [error, setError] = createSignal<string | null>(null);
  const initialView = (new URLSearchParams(location.search).get("view") as View) || "inventory";
  const [view, setView] = createSignal<View>(initialView);
  const [catalog, setCatalog] = createSignal<CatalogEntry[]>([]);
  const [menu, setMenu] = createSignal<{ x: number; y: number; id: string | null; source: Source } | null>(null);

  const dirty = () => snap()?.dirty ?? false;
  // ok() updates this window and notifies the others (container pop-outs).
  const ok = (s: Snapshot, msg: string) => { setSnap(s); setStatus(msg); setError(null); void broadcastChange(); };
  const fail = (e: unknown) => { setError(String(e)); setStatus("ERROR"); };

  const reload = async () => {
    try {
      setStatus("LOADING SAVE…");
      ok(await loadState(), "READY");
      setSelIds([]);
      setStatus(`READY · ${snap()?.savePath ?? ""}`);
    } catch (e) { fail(e); }
  };
  onMount(async () => {
    await reload();
    try { setCatalog(await listCatalog()); } catch (e) { /* catalog optional */ }
    // Refresh when a container pop-out window changes the shared save.
    const un = await onStateChanged(async () => { try { setSnap(await currentSnapshot()); } catch { /* */ } });
    onCleanup(() => un());
  });

  const allItems = createMemo<ItemView[]>(() => {
    const s = snap();
    return s ? [...s.inventory, ...s.equipment, ...s.shelter] : [];
  });
  const selected = createMemo(() => allItems().find((it) => it.id === selId()) ?? null);
  const selSource = createMemo<Source | null>(() => {
    const s = snap(); const id = selId();
    if (!s || !id) return null;
    if (s.inventory.some((i) => i.id === id)) return "inventory";
    if (s.equipment.some((i) => i.id === id)) return "equipment";
    if (s.shelter.some((i) => i.id === id)) return "shelter";
    return null;
  });
  const category = (it: ItemView | null) => (it ? it.visualName.split("/")[0] || "Item" : "");

  // Valid "add item" destinations: the vault and real on-character/inventory
  // containers. Shelter isn't shown in the UI, and the engine's meta containers
  // (Phantom/Buff) aren't real storage - exclude both so adds always land
  // somewhere visible.
  const addDestinations = createMemo(() => {
    const s = snap();
    if (!s) return [];
    return s.containers.filter(
      (c) => c.source !== "shelter" && !/phantom|buff|modifier/i.test(c.label),
    );
  });

  // The inventory is split into pages (stash tabs). `activePageId` is the tab
  // the user is viewing; it defaults to the first page and self-heals if the
  // current id vanishes (e.g. after switching saves).
  const [pickedPageId, setPickedPageId] = createSignal<string | null>(null);
  const activePageId = (): string | null => {
    const ps = snap()?.pages ?? [];
    const cur = pickedPageId();
    return ps.find((p) => p.id === cur)?.id ?? ps[0]?.id ?? null;
  };
  const vaultItems = createMemo<ItemView[]>(() => {
    const s = snap();
    const page = activePageId();
    if (!s || !page) return [];
    return s.inventory.filter((it) => it.parentId === page);
  });
  const vaultMeta = createMemo(() => {
    const items = vaultItems();
    const maxI = items.reduce((m, it) => Math.max(m, (it.i ?? -1) + 1), 0);
    const maxJ = items.reduce((m, it) => Math.max(m, (it.j ?? -1) + 1), 0);
    return { cols: Math.max(maxI, 8), rows: maxJ, count: items.length };
  });

  // ---- mutation handlers ----
  const onApply = async (v: { qty: number | null; condition: number | null; durability: number | null }) => {
    const id = selId(), src = selSource();
    if (!id || !src) return;
    try { ok(await applyItem(src, id, v.qty, v.condition, v.durability), `EDITED ${selected()?.name ?? ""} · staged`); }
    catch (e) { fail(e); }
  };
  const onRepair = async () => {
    const id = selId();
    if (!id) return;
    try { ok(await repairItems([id]), `REPAIRED ${selected()?.name ?? ""} · staged`); } catch (e) { fail(e); }
  };
  const onRepairAll = async () => {
    const s = snap();
    if (!s) return;
    const ids = [...s.inventory, ...s.equipment, ...s.shelter].map((it) => it.id);
    try { ok(await repairItems(ids), `REPAIRED ALL (${ids.length} items) · staged`); } catch (e) { fail(e); }
  };
  const onTopUpStacks = async () => {
    try { ok(await topUpStacks(), "TOPPED UP ALL STACKS · staged"); } catch (e) { fail(e); }
  };
  const onDelete = async () => {
    const ids = selIds();
    if (!ids.length) return;
    try { const s = await deleteItems(ids); setSelIds([]); ok(s, `DELETED ${ids.length} ITEM(S) · staged`); } catch (e) { fail(e); }
  };
  // ---- right-click copy / paste / delete + bulk on a multi-selection ----
  const onCopy = async (source: Source, id: string) => {
    try { ok(await copyItem(source, id), `COPIED ${snap()?.clipboard ?? ""}`); } catch (e) { fail(e); }
  };
  const onPaste = async (destSource: Source, destOwner: string) => {
    try { ok(await pasteItem(destSource, destOwner), "PASTED · staged"); } catch (e) { fail(e); }
  };
  const onDeleteIds = async (ids: string[]) => {
    if (!ids.length) return;
    try { const s = await deleteItems(ids); setSelIds([]); ok(s, `DELETED ${ids.length} · staged`); } catch (e) { fail(e); }
  };
  const onRepairIds = async (ids: string[]) => {
    if (!ids.length) return;
    try { ok(await repairItems(ids), `REPAIRED ${ids.length} · staged`); } catch (e) { fail(e); }
  };
  const onMoveToPage = async (id: string, pageId: string, n: number) => {
    try { ok(await moveToPage("inventory", id, pageId), `MOVED TO PAGE ${n} · staged`); } catch (e) { fail(e); }
  };
  const pasteIntoVault = (s: Snapshot): MenuItem => ({
    label: s.clipboard ? `Paste "${s.clipboard}" into page` : "Paste (clipboard empty)",
    disabled: !s.clipboard,
    action: () => onPaste("inventory", activePageId()!),
  });
  // "Move to page N" entries. Suppress a page only when EVERY selected item is
  // already on it; for a mixed selection, skip the items already on the target
  // page in the action so they don't get pointlessly relocated to a fresh slot.
  const moveToPageItems = (s: Snapshot, ids: string[]): MenuItem[] => {
    if (s.pages.length < 2) return [];
    const onPage = new Set(s.inventory.filter((it) => ids.includes(it.id)).map((it) => it.parentId));
    return s.pages
      .filter((pg) => !(onPage.size === 1 && onPage.has(pg.id)))
      .map((pg) => ({
        label: ids.length > 1 ? `Move ${ids.length} to Page ${pg.index}` : `Move to Page ${pg.index}`,
        action: async () => {
          const movable = ids.filter((id) => s.inventory.find((x) => x.id === id)?.parentId !== pg.id);
          for (const id of movable) await onMoveToPage(id, pg.id, pg.index);
        },
      }));
  };
  const menuItems = (): MenuItem[] => {
    const m = menu(); const s = snap();
    if (!m || !s) return [];
    // Right-click on empty space -> paste-only.
    if (m.id === null) {
      return activePageId() ? [pasteIntoVault(s)] : [];
    }
    const sel = selIds();
    // Right-clicking a member of a multi-selection -> bulk actions.
    if (sel.length > 1 && sel.includes(m.id)) {
      return [
        ...moveToPageItems(s, sel),
        { label: `Repair ${sel.length} items`, action: () => onRepairIds(sel) },
        { label: `Delete ${sel.length} items`, danger: true, action: () => onDeleteIds(sel) },
      ];
    }
    const id = m.id;
    const it = [...s.inventory, ...s.equipment, ...s.shelter].find((x) => x.id === id);
    const items: MenuItem[] = [
      { label: it?.isContainer ? "Copy (with contents)" : "Copy", action: () => onCopy(m.source, id) },
    ];
    if (activePageId()) items.push(pasteIntoVault(s));
    if (it?.isContainer && s.clipboard) {
      items.push({ label: `Paste "${s.clipboard}" into ${it.name}`, action: () => onPaste(m.source, id) });
    }
    // Move to other pages (only for inventory items).
    if (m.source === "inventory") items.push(...moveToPageItems(s, [id]));
    items.push({ label: "Delete", danger: true, action: () => onDeleteIds([id]) });
    return items;
  };
  const onMoveItem = async (id: string, i: number, j: number) => {
    try { ok(await moveItem("inventory", id, i, j), `MOVED → (${i},${j}) · staged`); } catch (e) { fail(e); }
  };
  const onSave = async () => {
    try {
      const r = await saveGame();
      setStatus(r.message + (r.backup ? `  ·  backup: ${r.backup.split(/[\\/]/).pop()}` : ""));
      setSnap(await currentSnapshot());
      if (!r.ok) setError(r.message);
    } catch (e) { fail(e); }
  };
  const onReload = async () => {
    try { ok(await reloadFromDisk(), "RELOADED FROM DISK"); setSelIds([]); } catch (e) { fail(e); }
  };
  const onAdd = async (a: {
    templateId: string; source: string; ownerId: string; quantity: number | null;
    count: number; condition: number | null; durability: number | null; gridWidth: number;
  }) => {
    try {
      ok(await addItems({ ...a, source: a.source as Source }), `ADDED ${a.count}× item · staged`);
      setView("inventory");
    } catch (e) { fail(e); }
  };
  const onSetAccount = async (a: Parameters<typeof setAccount>[0]) => {
    try { ok(await setAccount(a), "ACCOUNT UPDATED · staged"); } catch (e) { fail(e); }
  };
  const onSetSkill = async (id: number, level: number | null, nextGoal: number | null) => {
    try { ok(await setSkill(id, level, nextGoal), `SKILL #${id} UPDATED · staged`); } catch (e) { fail(e); }
  };
  // Double-click (or click the badge on) a container to open it in a pop-out.
  const openFrom = (src: Source) => (id: string) => {
    const arr = src === "inventory" ? snap()?.inventory : src === "equipment" ? snap()?.equipment : snap()?.shelter;
    const it = arr?.find((x) => x.id === id);
    if (it?.isContainer) {
      openContainerWindow(src, it.id, it.name)
        .then(() => setStatus(`OPENED ${it.name}`))
        .catch((e) => { setError(String(e)); setStatus("FAILED TO OPEN CONTAINER"); });
    }
  };

  const tab = (v: View, label: string) => (
    <button class="tab" classList={{ active: view() === v }} onClick={() => setView(v)}>{label}</button>
  );

  return (
    <div class="app">
      <div class="topbar">
        <span class="title">CARGO HUNTERS</span>
        {tab("inventory", "INVENTORY")}
        {tab("add", "ADD ITEMS")}
        {tab("character", "CHARACTER")}
        <span class="grow" />
        <span class="dirty" classList={{ on: dirty() }}>
          {dirty() ? "● UNSAVED STAGED CHANGES" : "NO UNSAVED CHANGES"}
        </span>
        <button onClick={onRepairAll} title="Repair, refill and top-off every item">REPAIR ALL</button>
        <button onClick={onTopUpStacks} title="Set every stack (everywhere, incl. containers) to its max">TOP UP STACKS</button>
        <button onClick={onReload}>RELOAD</button>
        <button class="primary" onClick={onSave}>SAVE</button>
      </div>

      <Show when={error()}>
        <div class="statusbar" style={{ color: "var(--danger)" }}>{error()}</div>
      </Show>

      <Show when={view() === "inventory"}>
        <div class="main">
          <div class="char-pane">
            <div class="pane-head">
              <span class="h">CHARACTER</span>
              <span class="sub">{snap()?.account.nickname ?? ""} · LVL {snap()?.account.level ?? "?"}</span>
            </div>
            <div class="scroll">
              <Show when={snap()}>
                <Paperdoll equipment={snap()!.equipment} selectedId={selId()}
                  onSelect={(id) => setSel(id)} onActivate={openFrom("equipment")}
                  onContextMenu={(id, x, y) => setMenu({ id, x, y, source: "equipment" })} />
              </Show>
            </div>
          </div>
          <div class="vault-pane">
            <div class="pane-head">
              <span class="h">VAULT</span>
              <Show when={(snap()?.pages.length ?? 0) > 1}>
                <div class="page-tabs">
                  <For each={snap()!.pages}>
                    {(pg) => (
                      <button class="ptab" classList={{ active: pg.id === activePageId() }}
                        title={`${pg.itemCount} items`} onClick={() => setPickedPageId(pg.id)}>
                        {roman(pg.index)}
                      </button>
                    )}
                  </For>
                </div>
              </Show>
              <span class="sub">{vaultMeta().cols}×{vaultMeta().rows} · {vaultMeta().count} ITEMS</span>
            </div>
            <div class="scroll">
              <Show when={snap()}>
                <VaultGrid items={vaultItems()} cols={vaultMeta().cols} selectedIds={selIds()}
                  onSelect={(id, additive) => (additive ? toggleSel(id) : setSel(id))}
                  onSelectBox={(ids) => setSelIds(ids)}
                  onMove={onMoveItem} onActivate={openFrom("inventory")}
                  onContextMenu={(id, x, y) => setMenu({ id, x, y, source: "inventory" })} />
              </Show>
            </div>
          </div>
        </div>
        <EditBar item={selected()} category={category(selected())}
          onApply={onApply} onRepair={onRepair} onDelete={onDelete} />
      </Show>

      <Show when={view() === "add"}>
        <div class="main">
          <Show when={snap()} fallback={<div class="statusbar">No save loaded.</div>}>
            <CatalogBrowser catalog={catalog()} containers={addDestinations()}
              onAdd={onAdd} onClose={() => setView("inventory")} />
          </Show>
        </div>
      </Show>

      <Show when={view() === "character"}>
        <div class="main">
          <Show when={snap()}>
            <CharacterPanel account={snap()!.account} onSetAccount={onSetAccount}
              onSetSkill={onSetSkill} onClose={() => setView("inventory")} />
          </Show>
        </div>
      </Show>

      <div class="statusbar">{status()}</div>

      <Show when={menu()}>
        {(m) => <ContextMenu x={m().x} y={m().y} items={menuItems()} onClose={() => setMenu(null)} />}
      </Show>
    </div>
  );
}
