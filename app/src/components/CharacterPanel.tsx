import { For, Show, createEffect, createSignal, on } from "solid-js";
import type { Account, SkillView } from "../api";
import { iconUrl } from "../icons";

interface Props {
  account: Account;
  onSetAccount: (a: {
    nickname: string | null;
    level: number | null;
    xp: number | null;
    nextGoal: number | null;
    skillPoints: number | null;
  }) => void;
  onSetSkill: (id: number, level: number | null, nextGoal: number | null) => void;
  onClose: () => void;
}

// Account fields (level/xp/goal/skill points) are i64 on the engine side:
// truncate to a non-negative integer in JS's safe range so a fractional or
// oversized value can't reject the IPC call with an opaque serde error.
const numOrNull = (s: string): number | null => {
  const t = s.trim();
  if (t === "") return null;
  const v = Number(t);
  if (!Number.isFinite(v)) return null;
  return Math.min(Math.max(0, Math.trunc(v)), Number.MAX_SAFE_INTEGER);
};

export default function CharacterPanel(p: Props) {
  const [nick, setNick] = createSignal("");
  const [level, setLevel] = createSignal("");
  const [xp, setXp] = createSignal("");
  const [goal, setGoal] = createSignal("");
  const [sp, setSp] = createSignal("");
  const [showDisabled, setShowDisabled] = createSignal(false);

  // Reseed when the account object is replaced (load / reload / after apply),
  // so the inputs and the skill lists stay in sync instead of freezing at mount.
  createEffect(on(() => p.account, (a) => {
    setNick(a.nickname ?? "");
    setLevel(a.level != null ? String(a.level) : "");
    setXp(a.xp != null ? String(a.xp) : "");
    setGoal(a.nextGoal != null ? String(a.nextGoal) : "");
    setSp(a.skillPoints != null ? String(a.skillPoints) : "");
  }));

  const active = () => p.account.skills.filter((s) => !s.disabled);
  const disabled = () => p.account.skills.filter((s) => s.disabled);

  return (
    <div class="charpanel">
      <div class="pane-head">
        <span class="h">CHARACTER &amp; SKILLS</span>
        <span class="sub"><button onClick={() => p.onClose()}>‹ BACK</button></span>
      </div>
      <div class="charpanel-body scroll">
        <section class="cp-card">
          <h3>ACCOUNT</h3>
          <label class="cp-row">NICKNAME<input type="text" value={nick()} onInput={(e) => setNick(e.currentTarget.value)} /></label>
          <label class="cp-row">LEVEL<input type="number" value={level()} onInput={(e) => setLevel(e.currentTarget.value)} /></label>
          <label class="cp-row">EXPERIENCE<input type="number" value={xp()} onInput={(e) => setXp(e.currentTarget.value)} /></label>
          <label class="cp-row">NEXT-LEVEL GOAL<input type="number" value={goal()} onInput={(e) => setGoal(e.currentTarget.value)} /></label>
          <label class="cp-row">SKILL POINTS<input type="number" value={sp()} onInput={(e) => setSp(e.currentTarget.value)} /></label>
          <button class="primary" onClick={() => p.onSetAccount({
            nickname: nick() !== (p.account.nickname ?? "") ? nick() : null,
            level: numOrNull(level()),
            xp: numOrNull(xp()),
            nextGoal: numOrNull(goal()),
            skillPoints: numOrNull(sp()),
          })}>APPLY ACCOUNT</button>
        </section>

        <section class="cp-card cp-skills">
          <h3>SKILLS</h3>
          <For each={active()}>{(sk) => <SkillRow sk={sk} onSet={p.onSetSkill} />}</For>
          <Show when={disabled().length}>
            <button class="cp-toggle" onClick={() => setShowDisabled(!showDisabled())}>
              {showDisabled() ? "▾" : "▸"} {disabled().length} deprecated skills
            </button>
            <Show when={showDisabled()}>
              <For each={disabled()}>{(sk) => <SkillRow sk={sk} onSet={p.onSetSkill} />}</For>
            </Show>
          </Show>
        </section>
      </div>
    </div>
  );
}

function SkillRow(p: { sk: SkillView; onSet: (id: number, l: number | null, g: number | null) => void }) {
  const [lvl, setLvl] = createSignal(p.sk.level != null ? String(p.sk.level) : "");
  createEffect(on(() => p.sk.level, () => setLvl(p.sk.level != null ? String(p.sk.level) : "")));
  return (
    <div class="cp-skill" classList={{ "cp-skill-off": p.sk.disabled }}>
      <Show when={p.sk.icon}>
        <img class="cp-skill-ico" src={iconUrl(p.sk.icon!)} draggable={false}
          onError={(e) => ((e.currentTarget as HTMLImageElement).style.visibility = "hidden")} />
      </Show>
      <span class="cp-skill-name">{p.sk.name}</span>
      <label class="fld">LVL<input class="narrow" type="number" placeholder="0" value={lvl()}
        onInput={(e) => setLvl(e.currentTarget.value)} /></label>
      <Show when={p.sk.maxLevel}><span class="cp-skill-max">/ {p.sk.maxLevel}</span></Show>
      <span class="grow" />
      {/* Only level matters; the per-skill XP goal is left untouched. */}
      <button onClick={() => p.onSet(p.sk.id, numOrNull(lvl()), null)}>SET</button>
    </div>
  );
}
