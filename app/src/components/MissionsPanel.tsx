import { For, Show, createMemo, createSignal, onMount } from "solid-js";
import { listMissions, type MissionsView, type MissionView } from "../api";

/** Read-only view of the player's missions, deciphered from the save's opaque
 * quest DataIds via the embedded quest catalog. The save stores no objective
 * progress, so this lists what's in progress (and tallies the rest). */
export default function MissionsPanel(p: { onClose: () => void }) {
  const [mv, setMv] = createSignal<MissionsView | null>(null);
  const [err, setErr] = createSignal<string | null>(null);
  const [showHidden, setShowHidden] = createSignal(false);

  onMount(async () => {
    try { setMv(await listMissions()); } catch (e) { setErr(String(e)); }
  });

  const visible = createMemo(() => (mv()?.active ?? []).filter((m) => !m.hidden));
  const hidden = createMemo(() => (mv()?.active ?? []).filter((m) => m.hidden));
  // Group visible missions by category for a tidy list.
  const groups = createMemo(() => {
    const byCat = new Map<string, MissionView[]>();
    for (const m of visible()) {
      const k = m.category || "OTHER";
      (byCat.get(k) ?? byCat.set(k, []).get(k)!).push(m);
    }
    return [...byCat.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  });

  return (
    <div class="charpanel">
      <div class="pane-head">
        <span class="h">MISSIONS</span>
        <span class="sub"><button onClick={() => p.onClose()}>‹ BACK</button></span>
      </div>
      <div class="charpanel-body scroll">
        <Show when={err()}><div class="modal-empty">{err()}</div></Show>
        <Show when={mv()} fallback={<div class="modal-empty">Reading missions…</div>}>
          <section class="cp-card" style={{ "max-width": "100%", flex: "1 1 100%" }}>
            <h3>IN PROGRESS · {visible().length} active{hidden().length ? ` (+${hidden().length} system)` : ""}</h3>
            <div class="mission-tally">
              <span><b>{mv()!.completedCount}</b> completed</span>
              <span><b>{mv()!.readyCount}</b> ready to claim</span>
              <span><b>{mv()!.availableCount}</b> available</span>
            </div>

            <Show when={visible().length} fallback={<div class="modal-empty">No player-facing missions in progress.</div>}>
              <For each={groups()}>
                {([cat, list]) => (
                  <div class="mission-group">
                    <div class="mission-cat">{cat}</div>
                    <For each={list}>
                      {(m) => (
                        <div class="mission-row" classList={{ unknown: !m.known }}>
                          <span class="mission-name">{m.name}</span>
                        </div>
                      )}
                    </For>
                  </div>
                )}
              </For>
            </Show>

            <Show when={hidden().length}>
              <button class="cp-toggle" onClick={() => setShowHidden(!showHidden())}>
                {showHidden() ? "▾" : "▸"} {hidden().length} system / telemetry quests
              </button>
              <Show when={showHidden()}>
                <For each={hidden()}>
                  {(m) => (
                    <div class="mission-row cp-skill-off">
                      <span class="mission-name">{m.name}</span>
                      <span class="mission-cat-inline">{m.category}</span>
                    </div>
                  )}
                </For>
              </Show>
            </Show>
          </section>
        </Show>
      </div>
    </div>
  );
}
