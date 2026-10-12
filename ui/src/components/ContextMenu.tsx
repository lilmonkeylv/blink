import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";

export type MenuItem = { label: string; run: () => void; disabled?: boolean } | "separator";

const [menu, setMenu] = createSignal<{ x: number; y: number; items: MenuItem[] } | null>(null);

export function openContextMenu(e: MouseEvent, items: MenuItem[]) {
  e.preventDefault();
  e.stopPropagation();
  setMenu({ x: e.clientX, y: e.clientY, items });
}

export function ContextMenuHost() {
  const close = () => setMenu(null);
  const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
  onMount(() => {
    window.addEventListener("mousedown", close);
    window.addEventListener("blur", close);
    window.addEventListener("keydown", onKey);
  });
  onCleanup(() => {
    window.removeEventListener("mousedown", close);
    window.removeEventListener("blur", close);
    window.removeEventListener("keydown", onKey);
  });

  return (
    <Show when={menu()}>
      {(m) => (
        <Portal>
          <div
            class="menu"
            style={{
              left: `${Math.min(m().x, window.innerWidth - 240)}px`,
              top: `${Math.min(m().y, window.innerHeight - m().items.length * 30 - 16)}px`,
            }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <For each={m().items}>
              {(item) =>
                item === "separator" ? (
                  <hr />
                ) : (
                  <button
                    disabled={item.disabled}
                    onClick={() => {
                      close();
                      item.run();
                    }}
                  >
                    {item.label}
                  </button>
                )
              }
            </For>
          </div>
        </Portal>
      )}
    </Show>
  );
}
