// Typed bindings over the Rust (Tauri) commands. Field names are camelCase to
// match the engine's `#[serde(rename_all = "camelCase")]` snapshot structs.

import { invoke } from "@tauri-apps/api/core";

export type Source = "inventory" | "equipment" | "shelter";

export interface ItemView {
  id: string;
  parentId: string | null;
  templateId: string;
  name: string;
  visualName: string;
  i: number | null;
  j: number | null;
  baseW: number;
  baseH: number;
  asmW: number | null;
  asmH: number | null;
  gridW: number;
  gridH: number;
  qty: number | null;
  conditionD: number | null;
  conditionMt: number | null;
  durability: number | null;
  durabilityMd: number | null;
  isContainer: boolean;
  slot: string;
}

export interface Container {
  label: string;
  source: Source;
  ownerItemId: string;
  gridWidth: number | null;
  gridHeight: number | null;
  templateId: string | null;
}

export interface SkillView {
  id: number;
  level: number | null;
  nextGoal: number | null;
  name: string;
  icon: string | null;
  maxLevel: number | null;
  disabled: boolean;
}

export interface Account {
  nickname: string | null;
  level: number | null;
  xp: number | null;
  nextGoal: number | null;
  skillPoints: number | null;
  skills: SkillView[];
}

export interface PageView {
  id: string;
  index: number;
  itemCount: number;
}

export interface Snapshot {
  savePath: string;
  pages: PageView[];
  containers: Container[];
  inventory: ItemView[];
  equipment: ItemView[];
  shelter: ItemView[];
  account: Account;
  dirty: boolean;
  clipboard: string | null;
}

// When running in a plain browser (dev/preview, no Tauri IPC) we fall back to a
// static snapshot fixture so the UI can be developed and screenshotted without
// launching the desktop shell. Inside Tauri this path is never taken.
const inTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function mockSnapshot(): Promise<Snapshot> {
  const res = await fetch("/mock-snapshot.json");
  return res.json();
}

export interface CatalogEntry {
  templateId: string;
  name: string;
  visualName: string;
  w: number;
  h: number;
  stackCapacity: number | null;
}

export interface SaveResult {
  ok: boolean;
  message: string;
  backup: string | null;
}

export function listCatalog(): Promise<CatalogEntry[]> {
  if (!inTauri()) return fetch("/mock-catalog.json").then((r) => r.json());
  return invoke("list_catalog");
}

export function defaultSavePath(): Promise<string | null> {
  if (!inTauri()) return Promise.resolve("(mock) offline.save");
  return invoke("default_save_path");
}

export function loadState(path?: string): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("load_state", { path: path ?? null });
}

export function currentSnapshot(): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("current_snapshot");
}

// ---- mutations (no-op in browser mock mode) ----

export function applyItem(
  source: Source,
  itemId: string,
  quantity: number | null,
  condition: number | null,
  durability: number | null,
): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("apply_item", { source, itemId, quantity, condition, durability });
}

export function repairItems(ids: string[]): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("repair_items", { ids });
}

export function topUpStacks(): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("top_up_stacks");
}

export function deleteItems(ids: string[]): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("delete_items", { ids });
}

export function copyItem(source: Source, itemId: string): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("copy_item", { source, itemId });
}

export function pasteItem(destSource: Source, destOwnerId: string): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("paste_item", { destSource, destOwnerId });
}

export function moveItem(source: Source, itemId: string, i: number, j: number): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("move_item", { source, itemId, i, j });
}

export function moveToPage(source: Source, itemId: string, destOwnerId: string): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("move_to_page", { source, itemId, destOwnerId });
}

export function splitStack(
  source: Source,
  itemId: string,
  splitQuantity: number,
  gridWidth: number | null,
): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("split_stack", { source, itemId, splitQuantity, gridWidth });
}

export function addItems(args: {
  templateId: string;
  source: Source;
  ownerId: string;
  quantity: number | null;
  count: number;
  condition: number | null;
  durability: number | null;
  gridWidth?: number; // accepted for compatibility; the engine derives the real width
}): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  const payload = { ...args };
  delete payload.gridWidth; // engine derives the real grid width itself
  return invoke("add_items", payload);
}

export function setAccount(args: {
  nickname: string | null;
  level: number | null;
  xp: number | null;
  nextGoal: number | null;
  skillPoints: number | null;
}): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("set_account", args);
}

export function setSkill(skillId: number, level: number | null, nextGoal: number | null): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("set_skill", { skillId, level, nextGoal });
}

export function saveGame(): Promise<SaveResult> {
  if (!inTauri()) return Promise.resolve({ ok: true, message: "(mock) not saved", backup: null });
  return invoke("save_game");
}

export function reloadFromDisk(): Promise<Snapshot> {
  if (!inTauri()) return mockSnapshot();
  return invoke("reload_from_disk");
}

// ---- multi-window: container pop-outs + cross-window sync ----

const STATE_CHANGED = "ch:state-changed";

/** Tauri window labels allow [a-zA-Z0-9-/:_]; uuids/source qualify already. */
function containerWindowLabel(source: Source, ownerId: string): string {
  return `container-${source}-${ownerId}`;
}

/** Open (or focus) a real OS pop-out window showing one container's grid. */
export async function openContainerWindow(source: Source, ownerId: string, label: string): Promise<void> {
  if (!inTauri()) {
    console.warn("container windows require the desktop app");
    return;
  }
  const { WebviewWindow } = await import("@tauri-apps/api/webviewWindow");
  const id = containerWindowLabel(source, ownerId);
  const existing = await WebviewWindow.getByLabel(id);
  if (existing) {
    await existing.setFocus();
    return;
  }
  const q = new URLSearchParams({ window: "container", source, owner: ownerId, label });
  const win = new WebviewWindow(id, {
    url: `index.html?${q.toString()}`,
    title: label,
    width: 620,
    height: 700,
    minWidth: 360,
    minHeight: 360,
  });
  // Surface failures to the caller instead of swallowing them.
  await new Promise<void>((resolve, reject) => {
    win.once("tauri://created", () => resolve());
    win.once("tauri://error", (e) => reject(new Error(`could not open window: ${JSON.stringify(e.payload)}`)));
  });
}

/** Tell every window the in-memory save changed so they can refresh. */
export async function broadcastChange(): Promise<void> {
  if (!inTauri()) return;
  const { emit } = await import("@tauri-apps/api/event");
  await emit(STATE_CHANGED);
}

/** Subscribe to cross-window change notifications; returns an unlisten fn. */
export async function onStateChanged(cb: () => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  const { listen } = await import("@tauri-apps/api/event");
  return listen(STATE_CHANGED, () => cb());
}
