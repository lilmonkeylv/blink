import { getCurrentWindow } from "@tauri-apps/api/window";
import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { MenuIcon } from "./Icons";

// the browser dev mock defines the IPC bridge but no window metadata
export const nativeWindow = !!(window as { __TAURI_INTERNALS__?: { metadata?: unknown } }).__TAURI_INTERNALS__?.metadata;
export const customTitleBar = nativeWindow && navigator.userAgent.includes("Windows");

export function WindowControls(p: { onMenu: (e: MouseEvent) => void }) {
  const win = getCurrentWindow();
  const [maximized, setMaximized] = createSignal(false);
  onMount(() => {
    const sync = () => win.isMaximized().then(setMaximized).catch(() => {});
    sync();
    const unlisten = win.onResized(sync);
    onCleanup(() => unlisten.then((u) => u()));
  });
  return (
    <div class="window-controls" onMouseDown={(e) => e.preventDefault()}>
      <button class="window-control app-menu-button" aria-label="Menu" title="Menu" onClick={(e) => p.onMenu(e)}>
        <MenuIcon />
      </button>
      <button class="window-control" aria-label="Minimize" onClick={() => win.minimize()}>
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 5h10" />
        </svg>
      </button>
      <button class="window-control" aria-label={maximized() ? "Restore" : "Maximize"} onClick={() => win.toggleMaximize()}>
        <Show
          when={maximized()}
          fallback={
            <svg viewBox="0 0 10 10" aria-hidden="true">
              <rect x="0.5" y="0.5" width="9" height="9" />
            </svg>
          }
        >
          <svg viewBox="0 0 10 10" aria-hidden="true">
            <rect x="0.5" y="2.5" width="7" height="7" />
            <path d="M2.5 2.5V0.5h7v7h-2" />
          </svg>
        </Show>
      </button>
      <button class="window-control close" aria-label="Close" onClick={() => win.close()}>
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M0 0l10 10M10 0L0 10" />
        </svg>
      </button>
    </div>
  );
}
