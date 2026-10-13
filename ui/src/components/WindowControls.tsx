import { getCurrentWindow } from "@tauri-apps/api/window";
import { createSignal, onCleanup, onMount, Show } from "solid-js";

export const customTitleBar = "__TAURI_INTERNALS__" in window && navigator.userAgent.includes("Windows");

export function WindowControls() {
  const win = getCurrentWindow();
  const [maximized, setMaximized] = createSignal(false);
  onMount(() => {
    const sync = () => win.isMaximized().then(setMaximized).catch(() => {});
    sync();
    const unlisten = win.onResized(sync);
    onCleanup(() => unlisten.then((u) => u()));
  });
  return (
    <div class="window-controls">
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
