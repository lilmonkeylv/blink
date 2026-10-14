import { createEffect, createSignal, on, onCleanup, Show } from "solid-js";
import { hookTitle } from "../lib/hooks";
import { setStore, store } from "../store";
import { ChevronDown, ChevronRight, CloseIcon, ErrorIcon, SuccessIcon } from "./Icons";

const PASSED_LINGER_MS = 6000;

export function HookPanel() {
  const [open, setOpen] = createSignal(false);
  const run = () => {
    const current = store.hookRun;
    return current && current.repo === store.repo?.id ? current : null;
  };
  let output: HTMLPreElement | undefined;
  let linger: number | undefined;

  createEffect(
    on(
      () => run()?.status,
      (status) => {
        clearTimeout(linger);
        if (!status) return;
        setOpen(status !== "passed");
        if (status !== "passed") return;
        const finished = store.hookRun;
        linger = window.setTimeout(() => {
          if (store.hookRun === finished && !open()) dismiss();
        }, PASSED_LINGER_MS);
      },
    ),
  );

  createEffect(
    on(
      () => run()?.lines.length,
      () => {
        if (output) output.scrollTop = output.scrollHeight;
      },
    ),
  );

  onCleanup(() => clearTimeout(linger));

  const dismiss = () => {
    clearTimeout(linger);
    setOpen(false);
    setStore({ hookRun: null, hookRetry: null });
  };

  return (
    <Show when={run()}>
      {(r) => (
        <div class="hook-panel" classList={{ [r().status]: true }}>
          <div class="hook-panel-header">
            <button class="hook-toggle" aria-expanded={open()} onClick={() => setOpen(!open())}>
              <Show when={r().status === "running"} fallback={r().status === "passed" ? <SuccessIcon /> : <ErrorIcon />}>
                <span class="spinner" aria-label="Running" />
              </Show>
              <span class="hook-title">{hookTitle(r())}</span>
              <Show when={r().lines.length}>
                <span class="muted small">{r().lines.length + r().dropped} lines</span>
              </Show>
              {open() ? <ChevronDown /> : <ChevronRight />}
            </button>
            <Show when={r().status !== "running"}>
              <button class="icon-button" aria-label="Dismiss hook output" onClick={dismiss}>
                <CloseIcon />
              </button>
            </Show>
          </div>
          <Show when={open()}>
            <pre class="hook-output" ref={output}>
              <Show when={r().dropped}>{`... ${r().dropped} earlier lines hidden\n`}</Show>
              {r().lines.length ? r().lines.join("\n") : r().status === "running" ? "Waiting for output" : "The hook printed nothing"}
            </pre>
          </Show>
          <Show when={r().status === "failed" && store.hookRetry}>
            {(retry) => (
              <div class="hook-actions">
                <button class="btn" onClick={() => retry().run()}>
                  {retry().label}
                </button>
              </div>
            )}
          </Show>
        </div>
      )}
    </Show>
  );
}
