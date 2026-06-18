import { Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import {
  addItems, applyItem, broadcastChange, currentSnapshot, deleteItems, listCatalog, loadState,
  moveItem, onStateChanged, openContainerWindow, reloadFromDisk, repairItems, saveGame,
  setAccount, setSkill, topUpStacks,
  type CatalogEntry, type ItemView, type Snapshot, type Source,
} from "./api";
import Paperdoll from "./components/Paperdoll";
import VaultGrid from "./components/VaultGrid";
import EditBar from "./components/EditBar";
import CatalogBrowser from "./components/CatalogBrowser";
import CharacterPanel from "./components/CharacterPanel";

type View = "inventory" | "add" | "character";

export default function App() {
  const [snap, setSnap] = createSignal<Snapshot | null>(null);
  const [selId, setSelId] = createSignal<string | null>(null);
  const [status, setStatus] = createSignal("LOADING…");
  const [error, setError] = createSignal<string | null>(null);
  const initialView = (new URLSearchParams(location.search).get("view") as View) || "inventory";
  const [view, setView] = createSignal<View>(initialView);
  const [catalog, setCatalog] = createSignal<CatalogEntry[]>([]);

  const dirty = () => snap()?.dirty ?? false;
  // ok() updates this window and notifies the others (container pop-outs).
  const ok = (s: Snapshot, msg: string) => { setSnap(s); setStatus(msg); setError(null); void broadcastChange(); };
  const fail = (e: unknown) => { setError(String(e)); setStatus("ERROR"); };

  const reload = async () => {
    try {
      setStatus("LOADING SAVE…");
      ok(await loadState(), "READY");
      setSelId(null);
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

  const vaultItems = createMemo<ItemView[]>(() => {
    const s = snap();
    if (!s || !s.backpackId) return [];
    return s.inventory.filter((it) => it.parentId === s.backpackId);
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
    const id = selId();
    if (!id) return;
    try { const s = await deleteItems([id]); setSelId(null); ok(s, "DELETED ITEM · staged"); } catch (e) { fail(e); }
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
    try { ok(await reloadFromDisk(), "RELOADED FROM DISK"); setSelId(null); } catch (e) { fail(e); }
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
                  onSelect={setSelId} onActivate={openFrom("equipment")} />
              </Show>
            </div>
          </div>
          <div class="vault-pane">
            <div class="pane-head">
              <span class="h">VAULT</span>
              <span class="sub">{vaultMeta().cols}×{vaultMeta().rows} · {vaultMeta().count} ITEMS</span>
            </div>
            <div class="scroll">
              <Show when={snap()}>
                <VaultGrid items={vaultItems()} cols={vaultMeta().cols} selectedId={selId()}
                  onSelect={setSelId} onMove={onMoveItem} onActivate={openFrom("inventory")} />
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
    </div>
  );
}
