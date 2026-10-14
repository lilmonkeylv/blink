import { createEffect, createSignal, For, on, onCleanup, Show } from "solid-js";
import { Dynamic } from "solid-js/web";
import { dismiss, store, type Banner } from "../store";
import { ChevronRight, CloseIcon, ErrorIcon, InfoIcon, SuccessIcon } from "./Icons";

const MAX_VISIBLE = 3;

const TONE_ICONS = { success: SuccessIcon, error: ErrorIcon, info: InfoIcon };

function Toast(p: { banner: Banner }) {
  const [hovered, setHovered] = createSignal(false);
  const [focused, setFocused] = createSignal(false);
  const held = () => hovered() || focused();
  let remaining = p.banner.duration ?? 0;
  let startedAt = 0;
  let timer: number | undefined;

  createEffect(
    on(held, (paused) => {
      if (!p.banner.duration || p.banner.leaving) return;
      if (paused) {
        clearTimeout(timer);
        remaining -= Date.now() - startedAt;
      } else {
        startedAt = Date.now();
        timer = window.setTimeout(() => dismiss(p.banner.id), remaining);
      }
    }),
  );
  onCleanup(() => clearTimeout(timer));

  const close = () => {
    p.banner.onDismiss?.();
    dismiss(p.banner.id);
  };

  return (
    <div class="toast-slot" classList={{ leaving: p.banner.leaving }}>
      <div class="toast-clip">
        <div
          class={`toast ${p.banner.tone}`}
          role={p.banner.tone === "error" ? "alert" : "status"}
          onMouseEnter={() => setHovered(true)}
          onMouseLeave={() => setHovered(false)}
          onFocusIn={() => setFocused(true)}
          onFocusOut={(e) => !e.currentTarget.contains(e.relatedTarget as Node) && setFocused(false)}
        >
          <Dynamic component={TONE_ICONS[p.banner.tone]} class="toast-icon" />
          <div class="toast-body">
            <div class="toast-message">{p.banner.message}</div>
            <Show when={p.banner.detail}>
              <details class="toast-detail">
                <summary>
                  <ChevronRight />
                  Show Git output
                </summary>
                <pre>{p.banner.detail}</pre>
              </details>
            </Show>
            <Show when={p.banner.action}>
              {(action) => (
                <div class="toast-actions">
                  <button
                    type="button"
                    class="link-button"
                    onClick={() => {
                      dismiss(p.banner.id);
                      action().run();
                    }}
                  >
                    {action().label}
                  </button>
                </div>
              )}
            </Show>
          </div>
          <button type="button" class="icon-button toast-close" aria-label="Dismiss" onClick={close}>
            <CloseIcon />
          </button>
          <Show when={p.banner.duration}>
            {(duration) => <span class="toast-progress" classList={{ paused: held() }} style={{ "animation-duration": `${duration()}ms` }} />}
          </Show>
        </div>
      </div>
    </div>
  );
}

export function Toasts() {
  return (
    <div class="toasts">
      <For each={store.banners.slice(-MAX_VISIBLE)}>{(banner) => <Toast banner={banner} />}</For>
    </div>
  );
}
