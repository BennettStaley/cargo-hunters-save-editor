import { For, createSignal, onCleanup, onMount } from "solid-js";

export interface MenuItem {
  label: string;
  disabled?: boolean;
  danger?: boolean;
  action: () => void;
}

/** A small right-click menu anchored at (x, y); closes on outside click / Esc. */
export default function ContextMenu(p: { x: number; y: number; items: MenuItem[]; onClose: () => void }) {
  let el: HTMLDivElement | undefined;
  const [pos, setPos] = createSignal({ x: p.x, y: p.y });
  onMount(() => {
    const onDown = (e: PointerEvent) => {
      if (el && !el.contains(e.target as Node)) p.onClose();
    };
    const onKey = (e: KeyboardEvent) => { if (e.key === "Escape") p.onClose(); };
    document.addEventListener("pointerdown", onDown, true);
    document.addEventListener("keydown", onKey);
    // Clamp into the viewport so the menu never clips off a small pop-out window.
    if (el) {
      const r = el.getBoundingClientRect();
      setPos({
        x: Math.max(4, Math.min(p.x, window.innerWidth - r.width - 4)),
        y: Math.max(4, Math.min(p.y, window.innerHeight - r.height - 4)),
      });
    }
    onCleanup(() => {
      document.removeEventListener("pointerdown", onDown, true);
      document.removeEventListener("keydown", onKey);
    });
  });
  return (
    <div class="ctxmenu" ref={el} style={{ left: `${pos().x}px`, top: `${pos().y}px` }}>
      <For each={p.items}>
        {(it) => (
          <button
            class="ctxmenu-item"
            classList={{ danger: it.danger }}
            disabled={it.disabled}
            onClick={() => { if (!it.disabled) { it.action(); p.onClose(); } }}
          >
            {it.label}
          </button>
        )}
      </For>
    </div>
  );
}
