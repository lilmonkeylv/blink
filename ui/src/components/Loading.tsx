import { createSignal, onCleanup, Show } from "solid-js";

const SHOW_AFTER_MS = 150;

export function Loading(p: { label?: string }) {
  const [visible, setVisible] = createSignal(false);
  const timer = setTimeout(() => setVisible(true), SHOW_AFTER_MS);
  onCleanup(() => clearTimeout(timer));
  return (
    <div class="empty" aria-busy="true">
      <Show when={visible()}>
        <div class="loading">
          <span class="spinner large" />
          <span class="muted">{p.label ?? "Loading"}…</span>
        </div>
      </Show>
    </div>
  );
}
