import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";
import { ChevronRight } from "./Icons";

export type MenuItem =
  | { label: string; run: () => void; disabled?: boolean; hint?: string }
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
let returnFocus: HTMLElement | null = null;

export function openContextMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const target = e.currentTarget;
  if (e.detail === 0 && target instanceof HTMLElement) {
    const rect = target.getBoundingClientRect();
    setMenu({ x: rect.left, y: rect.bottom + 2, flipX: rect.right, items });
  } else {
    setMenu({ x: e.clientX, y: e.clientY, items });
  }
}

function menuButtons(el: HTMLElement) {
  return [...el.querySelectorAll<HTMLButtonElement>(":scope > button:not(:disabled)")];
}

function MenuPanel(p: { menu: OpenMenu; onClose: () => void; onBack?: () => void; focusFirst: boolean }) {
  let el!: HTMLDivElement;
  const [submenu, setSubmenu] = createSignal<{ index: number; menu: OpenMenu; focusFirst: boolean; trigger: HTMLElement } | null>(null);
  onMount(() => {
    const { width, height } = el.getBoundingClientRect();
    const left = p.menu.x + width > window.innerWidth - EDGE ? Math.max(EDGE, (p.menu.flipX ?? p.menu.x) - width) : p.menu.x;
    const top = p.menu.y + height > window.innerHeight - EDGE ? Math.max(EDGE, window.innerHeight - EDGE - height) : p.menu.y;
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
    el.style.visibility = "visible";
    if (p.focusFirst) (menuButtons(el)[0] ?? el).focus();
  });
  const openSubmenu = (index: number, items: MenuItem[], target: HTMLElement, focusFirst = false) => {
    if (submenu()?.index === index && !focusFirst) return;
    const rect = target.getBoundingClientRect();
    setSubmenu({ index, focusFirst, trigger: target, menu: { x: rect.right + 4, y: rect.top - 5, flipX: rect.left - 4, items } });
  };
  const closeSubmenu = () => {
    const open = submenu();
    if (!open) return;
    setSubmenu(null);
    open.trigger.focus();
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.target instanceof HTMLElement && e.target.closest(".menu") !== el) return;
    const buttons = menuButtons(el);
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const focusAt = (i: number) => buttons[(i + buttons.length) % buttons.length]?.focus();
    const current = buttons[index];
    if (e.key === "ArrowDown") focusAt(index + 1);
    else if (e.key === "ArrowUp") focusAt(index < 0 ? -1 : index - 1);
    else if (e.key === "Home") focusAt(0);
    else if (e.key === "End") focusAt(-1);
    else if (e.key === "ArrowRight" && current?.classList.contains("submenu")) current.click();
    else if (e.key === "ArrowLeft" && p.onBack) p.onBack();
    else if (e.key === "Tab") p.onClose();
    else return;
    e.preventDefault();
    e.stopPropagation();
  };

  return (
    <div
      ref={el}
      class="menu"
      role="menu"
      tabIndex={-1}
      style={{
        left: `${p.menu.x}px`,
        top: `${p.menu.y}px`,
        visibility: "hidden",
        "max-height": `${window.innerHeight - 2 * EDGE}px`,
      }}
      onMouseDown={(e) => e.stopPropagation()}
      onKeyDown={onKeyDown}
    >
      <For each={p.menu.items}>
        {(item, index) =>
          item === "separator" ? (
            <hr role="separator" />
          ) : "items" in item ? (
            <button
              class="submenu"
              classList={{ open: submenu()?.index === index() }}
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={submenu()?.index === index()}
              disabled={item.disabled}
              onMouseEnter={(e) => {
                e.currentTarget.focus();
                openSubmenu(index(), item.items, e.currentTarget);
              }}
              onClick={(e) => openSubmenu(index(), item.items, e.currentTarget, e.detail === 0)}
            >
              <span class="menu-label">{item.label}</span>
              <ChevronRight />
            </button>
          ) : (
            <button
              role="menuitem"
              disabled={item.disabled}
              onMouseEnter={(e) => {
                e.currentTarget.focus();
                setSubmenu(null);
              }}
              onClick={() => {
                p.onClose();
                item.run();
              }}
            >
              <span class="menu-label">{item.label}</span>
              <Show when={item.hint}>
                <span class="menu-hint">{item.hint}</span>
              </Show>
            </button>
          )
        }
      </For>
      <Show when={submenu()} keyed>
        {(open) => (
          <Portal>
            <MenuPanel menu={open.menu} onClose={p.onClose} onBack={closeSubmenu} focusFirst={open.focusFirst} />
          </Portal>
        )}
      </Show>
    </div>
  );
}

export function ContextMenuHost() {
  const close = () => {
    if (!menu()) return;
    setMenu(null);
    if (returnFocus?.isConnected) returnFocus.focus();
    returnFocus = null;
  };
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
          <MenuPanel menu={m} onClose={close} focusFirst />
        </Portal>
      )}
    </Show>
  );
}
