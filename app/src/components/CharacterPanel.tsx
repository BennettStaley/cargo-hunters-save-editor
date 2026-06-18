import { For, createSignal } from "solid-js";
import type { Account } from "../api";

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

const numOrNull = (s: string): number | null => {
  const t = s.trim();
  if (t === "") return null;
  const v = Number(t);
  return Number.isFinite(v) ? v : null;
};

export default function CharacterPanel(p: Props) {
  const a = p.account;
  const [nick, setNick] = createSignal(a.nickname ?? "");
  const [level, setLevel] = createSignal(a.level != null ? String(a.level) : "");
  const [xp, setXp] = createSignal(a.xp != null ? String(a.xp) : "");
  const [goal, setGoal] = createSignal(a.nextGoal != null ? String(a.nextGoal) : "");
  const [sp, setSp] = createSignal(a.skillPoints != null ? String(a.skillPoints) : "");

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
            nickname: nick() !== (a.nickname ?? "") ? nick() : null,
            level: numOrNull(level()),
            xp: numOrNull(xp()),
            nextGoal: numOrNull(goal()),
            skillPoints: numOrNull(sp()),
          })}>APPLY ACCOUNT</button>
        </section>

        <section class="cp-card">
          <h3>SKILLS ({a.skills.length})</h3>
          <For each={a.skills}>
            {(sk) => <SkillRow id={sk.id} level={sk.level} nextGoal={sk.nextGoal} onSet={p.onSetSkill} />}
          </For>
        </section>
      </div>
    </div>
  );
}

function SkillRow(p: { id: number; level: number | null; nextGoal: number | null; onSet: (id: number, l: number | null, g: number | null) => void }) {
  const [lvl, setLvl] = createSignal(p.level != null ? String(p.level) : "");
  const [goal, setGoal] = createSignal(p.nextGoal != null ? String(p.nextGoal) : "");
  return (
    <div class="cp-skill">
      <span class="cp-skill-id">SKILL #{p.id}</span>
      <label class="fld">LVL<input class="narrow" type="number" value={lvl()} onInput={(e) => setLvl(e.currentTarget.value)} /></label>
      <label class="fld">GOAL<input type="number" value={goal()} onInput={(e) => setGoal(e.currentTarget.value)} /></label>
      <button onClick={() => p.onSet(p.id, numOrNull(lvl()), numOrNull(goal()))}>SET</button>
    </div>
  );
}
