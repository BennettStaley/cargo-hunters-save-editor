import { For, Show, createMemo, createSignal, onMount } from "solid-js";
import { listMissions, skipMissions, type MissionsView, type MissionView, type Snapshot } from "../api";

/** Missions deciphered from the save's opaque quest DataIds. The save stores no
 * objective progress, so missions can't be "partly done" - but they CAN be
 * skipped: completing one banks its XP (with level-up) and drops its item
 * rewards into the vault, all staged in memory until SAVE. */
export default function MissionsPanel(p: { onSaveChanged: (s: Snapshot) => void; onClose: () => void }) {
  const [mv, setMv] = createSignal<MissionsView | null>(null);
  const [err, setErr] = createSignal<string | null>(null);
  const [msg, setMsg] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [showHidden, setShowHidden] = createSignal(false);

  const refresh = async () => {
    try { setMv(await listMissions()); } catch (e) { setErr(String(e)); }
  };
  onMount(refresh);

  const visible = createMemo(() => (mv()?.active ?? []).filter((m) => !m.hidden));
  const hidden = createMemo(() => (mv()?.active ?? []).filter((m) => m.hidden));
  const claimable = createMemo(() => visible().filter((m) => m.claimable));
  const groups = createMemo(() => {
    const byCat = new Map<string, MissionView[]>();
    for (const m of visible()) {
      const k = m.category || "OTHER";
      (byCat.get(k) ?? byCat.set(k, []).get(k)!).push(m);
    }
    return [...byCat.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  });

  const skip = async (ids: string[], label: string) => {
    if (!ids.length || busy()) return;
    setBusy(true);
    try {
      const snap = await skipMissions(ids);
      p.onSaveChanged(snap); // update the main window's account/inventory + broadcast
      await refresh();
      setMsg(`${label} · staged (review on the Character & Inventory tabs, then SAVE)`);
    } catch (e) { setErr(String(e)); }
    finally { setBusy(false); }
  };

  return (
    <div class="charpanel">
      <div class="pane-head">
        <span class="h">MISSIONS</span>
        <span class="sub"><button onClick={() => p.onClose()}>‹ BACK</button></span>
      </div>
      <div class="charpanel-body scroll">
        <Show when={err()}><div class="modal-empty" style={{ color: "var(--danger)" }}>{err()}</div></Show>
        <Show when={mv()} fallback={<div class="modal-empty">Reading missions…</div>}>
          <section class="cp-card" style={{ "max-width": "100%", flex: "1 1 100%" }}>
            <div class="mission-head">
              <h3 style={{ margin: 0 }}>IN PROGRESS · {visible().length} active{hidden().length ? ` (+${hidden().length} system)` : ""}</h3>
              <Show when={claimable().length}>
                <button class="primary" disabled={busy()}
                  onClick={() => skip(claimable().map((m) => m.id), `Skipped & claimed ${claimable().length} missions`)}>
                  SKIP ALL &amp; CLAIM ({claimable().length})
                </button>
              </Show>
            </div>
            <div class="mission-tally">
              <span><b>{mv()!.completedCount}</b> completed</span>
              <span><b>{mv()!.readyCount}</b> ready to claim</span>
              <span><b>{mv()!.availableCount}</b> available</span>
            </div>
            <Show when={msg()}><div class="mission-msg">{msg()}</div></Show>

            <Show when={visible().length} fallback={<div class="modal-empty">No player-facing missions in progress.</div>}>
              <For each={groups()}>
                {([cat, list]) => (
                  <div class="mission-group">
                    <div class="mission-cat">{cat}</div>
                    <For each={list}>
                      {(m) => (
                        <div class="mission-row" classList={{ unknown: !m.known }}>
                          <span class="mission-name">{m.name}</span>
                          <span class="mission-reward">{m.reward}</span>
                          <button disabled={busy()}
                            title={m.claimable ? "Complete and bank the reward" : "Complete (no reward on this stage)"}
                            onClick={() => skip([m.id], `Skipped ${m.name}`)}>
                            {m.claimable ? "SKIP & CLAIM" : "SKIP"}
                          </button>
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
