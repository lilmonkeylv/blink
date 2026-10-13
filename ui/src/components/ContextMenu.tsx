import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { ChevronRight } from "./Icons";

export type MenuItem =
  | { label: string; run: () => void; disabled?: boolean }
  | { label: string; items: MenuItem[]; disabled?: boolean }
  | "separator";

interface OpenMenu {
  x: number;
  y: number;
  flipX?: number;
  items: MenuItem[];
}

type MenuAction = Extract<MenuItem, { run: () => void }>;

export function menuActions(items: MenuItem[]): MenuAction[] {
  return items.flatMap((item) => {
    if (item === "separator" || item.disabled) return [];
    return "items" in item ? menuActions(item.items) : [item];
  });
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
  const [submenu, setSubmenu] = createSignal<{ index: number; menu: OpenMenu } | null>(null);
  onMount(() => {
    const { width, height } = el.getBoundingClientRect();
    const left = p.menu.x + width > window.innerWidth - EDGE ? Math.max(EDGE, (p.menu.flipX ?? p.menu.x) - width) : p.menu.x;
    const top = p.menu.y + height > window.innerHeight - EDGE ? Math.max(EDGE, window.innerHeight - EDGE - height) : p.menu.y;
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
    el.style.visibility = "visible";
  });
  const openSubmenu = (index: number, items: MenuItem[], target: HTMLElement) => {
    if (submenu()?.index === index) return;
    const rect = target.getBoundingClientRect();
    setSubmenu({ index, menu: { x: rect.right + 4, y: rect.top - 5, flipX: rect.left - 4, items } });
  };
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
        {(item, index) =>
          item === "separator" ? (
            <hr />
          ) : "items" in item ? (
            <button
              class="submenu"
              classList={{ open: submenu()?.index === index() }}
              disabled={item.disabled}
              onMouseEnter={(e) => openSubmenu(index(), item.items, e.currentTarget)}
              onClick={(e) => openSubmenu(index(), item.items, e.currentTarget)}
            >
              <span>{item.label}</span>
              <ChevronRight />
            </button>
          ) : (
            <button
              disabled={item.disabled}
              onMouseEnter={() => setSubmenu(null)}
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
      <Show when={submenu()} keyed>
        {(open) => (
          <Portal>
            <MenuPanel menu={open.menu} onClose={p.onClose} />
          </Portal>
        )}
      </Show>
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
