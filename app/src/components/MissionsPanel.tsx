import { For, Show, createMemo, createSignal, onMount } from "solid-js";
import { addMissionItem, listMissions, type MissionsView, type MissionView, type ReqItemView, type Snapshot } from "../api";

/** Read-only view of in-progress missions, deciphered from the save's opaque
 * quest DataIds. It does NOT touch quest state - instead it shows what each
 * mission requires (and what you currently hold) and lets you ADD the required
 * materials to your vault, so you can hand them in and complete it in-game. */
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
  const groups = createMemo(() => {
    const byCat = new Map<string, MissionView[]>();
    for (const m of visible()) {
      const k = m.category || "OTHER";
      (byCat.get(k) ?? byCat.set(k, []).get(k)!).push(m);
    }
    return [...byCat.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  });
  const shortfall = (r: ReqItemView) => Math.max(0, r.need - r.have);
  const missionNeeds = (m: MissionView) =>
    m.objectives.flatMap((o) => o.items).filter((r) => shortfall(r) > 0);

  const add = async (items: { templateId: string; count: number }[], label: string) => {
    if (!items.length || busy()) return;
    setBusy(true);
    try {
      let snap: Snapshot | null = null;
      for (const it of items) snap = await addMissionItem(it.templateId, it.count);
      if (snap) p.onSaveChanged(snap); // update Inventory tab + broadcast
      await refresh();                  // recompute have-counts
      setMsg(`${label} · added to the vault, staged (then SAVE)`);
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
            <h3>IN PROGRESS · {visible().length} active{hidden().length ? ` (+${hidden().length} system)` : ""}</h3>
            <div class="mission-tally">
              <span><b>{mv()!.completedCount}</b> completed</span>
              <span><b>{mv()!.readyCount}</b> ready to claim</span>
              <span><b>{mv()!.availableCount}</b> available</span>
            </div>
            <div class="mission-note">Requirements are obtained in-raid; add what you're missing here, then hand it in in-game. Nothing here changes quest state.</div>
            <Show when={msg()}><div class="mission-msg">{msg()}</div></Show>

            <Show when={visible().length} fallback={<div class="modal-empty">No player-facing missions in progress.</div>}>
              <For each={groups()}>
                {([cat, list]) => (
                  <div class="mission-group">
                    <div class="mission-cat">{cat}</div>
                    <For each={list}>
                      {(m) => (
                        <div class="mission-block">
                          <div class="mission-title">
                            <span class="mission-name">{m.name}</span>
                            <Show when={m.reward}><span class="mission-reward">reward: {m.reward}</span></Show>
                            <Show when={missionNeeds(m).length > 1}>
                              <button class="m-addall" disabled={busy()}
                                onClick={() => add(missionNeeds(m).map((r) => ({ templateId: r.templateId, count: shortfall(r) })), `Added all materials for ${m.name}`)}>
                                ADD ALL MATERIALS
                              </button>
                            </Show>
                          </div>
                          <For each={m.objectives}>
                            {(o) => (
                              <div class="objective">
                                <Show when={o.desc}><div class="obj-desc">• {o.desc}</div></Show>
                                <For each={o.items}>
                                  {(r) => (
                                    <div class="req">
                                      <span class="req-name">{r.name || r.templateId.slice(0, 8)}</span>
                                      <span class="req-count" classList={{ ok: r.have >= r.need }}>
                                        have {r.have} / need {r.need}
                                      </span>
                                      <Show when={shortfall(r) > 0}
                                        fallback={<span class="req-have">✓</span>}>
                                        <button class="req-add" disabled={busy()}
                                          onClick={() => add([{ templateId: r.templateId, count: shortfall(r) }], `Added ${shortfall(r)}x ${r.name || "item"}`)}>
                                          ADD {shortfall(r)}
                                        </button>
                                      </Show>
                                    </div>
                                  )}
                                </For>
                              </div>
                            )}
                          </For>
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
