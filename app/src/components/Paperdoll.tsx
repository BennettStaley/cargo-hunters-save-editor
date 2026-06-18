import { For, Show, createMemo } from "solid-js";
import type { ItemView } from "../api";
import ItemTile from "./ItemTile";

interface Props {
  equipment: ItemView[]; // all equipment-source items
  selectedId: string | null;
  onSelect: (id: string) => void;
  onActivate?: (id: string) => void; // double-click a container -> pop-out
}

interface SlotDef {
  kind: string;
  label: string;
  cx: number;
  cy: number;
  size: number;
}

// Positions are centres within the DOLL_W x DOLL_H box (must match .doll in
// theme.css). Body parts sit on the silhouette anatomically; gear rides in the
// left/right rails. Kept inside the box so nothing clips at the pane edge.
export const DOLL_W = 460;
export const DOLL_H = 600;

const BODY: SlotDef[] = [
  { kind: "bodypart_head", label: "HEAD", cx: 230, cy: 56, size: 76 },
  { kind: "bodypart_torso", label: "TORSO", cx: 230, cy: 212, size: 78 },
  { kind: "bodypart_arm_left", label: "L · ARM", cx: 138, cy: 198, size: 72 },
  { kind: "bodypart_arm_right", label: "R · ARM", cx: 322, cy: 198, size: 72 },
  { kind: "bodypart_leg_left", label: "L · LEG", cx: 176, cy: 432, size: 72 },
  { kind: "bodypart_leg_right", label: "R · LEG", cx: 284, cy: 432, size: 72 },
];

const GEAR: SlotDef[] = [
  { kind: "gear_helmet", label: "HELMET", cx: 40, cy: 58, size: 66 },
  { kind: "gear_vest", label: "ARMOR", cx: 40, cy: 148, size: 66 },
  { kind: "gear_vest", label: "ARMOR", cx: 40, cy: 238, size: 66 },
  { kind: "gear_backpack", label: "BACKPACK", cx: 40, cy: 328, size: 66 },
  { kind: "gear_safestash", label: "STASH", cx: 420, cy: 58, size: 66 },
  { kind: "gear_tool", label: "TOOL", cx: 420, cy: 148, size: 66 },
  { kind: "gear_weapon", label: "PRIMARY", cx: 420, cy: 238, size: 66 },
  { kind: "gear_weapon", label: "SECONDARY", cx: 420, cy: 328, size: 66 },
  { kind: "gear_melee", label: "MELEE", cx: 420, cy: 432, size: 66 },
];

interface Placed extends SlotDef {
  item: ItemView | null;
}

export default function Paperdoll(p: Props) {
  const placed = createMemo(() => {
    const pool = p.equipment.filter((it) => it.parentId === null && it.slot !== "meta");
    const used = new Set<string>();
    const fill = (defs: SlotDef[]): Placed[] =>
      defs.map((d) => {
        const item = pool.find((it) => it.slot === d.kind && !used.has(it.id)) ?? null;
        if (item) used.add(item.id);
        return { ...d, item };
      });
    const body = fill(BODY);
    const gear = fill(GEAR);
    const extras = pool.filter((it) => !used.has(it.id));
    return { body, gear, extras };
  });

  const Slot = (s: Placed) => (
    <div
      class="slot"
      classList={{ filled: !!s.item }}
      style={{
        left: `${s.cx - s.size / 2}px`,
        top: `${s.cy - s.size / 2}px`,
        width: `${s.size}px`,
        height: `${s.size}px`,
      }}
    >
      <div class="slot-lbl">{s.label}</div>
      <Show when={s.item}>
        <ItemTile
          item={s.item!}
          left={0}
          top={0}
          width={s.size}
          height={s.size}
          selected={p.selectedId === s.item!.id}
          onSelect={p.onSelect}
          onActivate={p.onActivate}
        />
      </Show>
    </div>
  );

  return (
    <div class="doll" style={{ width: `${DOLL_W}px`, height: `${DOLL_H}px` }}>
      <img class="silhouette" src="/sprites/BodyHUD.png" draggable={false} />
      <For each={placed().body}>{Slot}</For>
      <For each={placed().gear}>{Slot}</For>
      {/* Any equipment that didn't match a known slot, laid out below. */}
      <Show when={placed().extras.length}>
        <div style={{ position: "absolute", left: "8px", right: "8px", bottom: "2px",
          display: "flex", "flex-wrap": "wrap", gap: "6px", "justify-content": "center" }}>
          <For each={placed().extras}>
            {(it) => (
              <div class="slot filled" style={{ position: "relative", width: "64px", height: "64px" }}>
                <ItemTile item={it} left={0} top={0} width={64} height={64}
                  selected={p.selectedId === it.id} onSelect={p.onSelect} onActivate={p.onActivate} />
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
