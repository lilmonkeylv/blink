import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";

export type MenuItem = { label: string; run: () => void; disabled?: boolean } | "separator";

interface OpenMenu {
  x: number;
  y: number;
  items: MenuItem[];
}

const EDGE = 8;

const [menu, setMenu] = createSignal<OpenMenu | null>(null);

export function openContextMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  setMenu({ x: e.clientX, y: e.clientY, items });
}

function MenuPanel(p: { menu: OpenMenu; onClose: () => void }) {
  let el!: HTMLDivElement;
  onMount(() => {
    const { width, height } = el.getBoundingClientRect();
    const left = p.menu.x + width > window.innerWidth - EDGE ? Math.max(EDGE, p.menu.x - width) : p.menu.x;
    const top = p.menu.y + height > window.innerHeight - EDGE ? Math.max(EDGE, window.innerHeight - EDGE - height) : p.menu.y;
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
    el.style.visibility = "visible";
  });
  return (
    <div
      ref={el}
      class="menu"
      style={{
        left: `${p.menu.x}px`,
        top: `${p.menu.y}px`,
        visibility: "hidden",
        "max-height": `${window.innerHeight - 2 * EDGE}px`,
      }}
      onMouseDown={(e) => e.stopPropagation()}
    >
      <For each={p.menu.items}>
        {(item) =>
          item === "separator" ? (
            <hr />
          ) : (
            <button
              disabled={item.disabled}
              onClick={() => {
                p.onClose();
                item.run();
              }}
            >
              {item.label}
            </button>
          )
        }
      </For>
    </div>
  );
}

export function ContextMenuHost() {
  const close = () => setMenu(null);
  const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
  onMount(() => {
    window.addEventListener("mousedown", close);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    window.addEventListener("keydown", onKey);
  });
  onCleanup(() => {
    window.removeEventListener("mousedown", close);
    window.removeEventListener("blur", close);
    window.removeEventListener("resize", close);
    window.removeEventListener("keydown", onKey);
  });

  return (
    <Show when={menu()} keyed>
      {(m) => (
        <Portal>
          <MenuPanel menu={m} onClose={close} />
        </Portal>
      )}
    </Show>
  );
}
