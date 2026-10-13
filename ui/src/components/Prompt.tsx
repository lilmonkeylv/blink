import { createSignal, For, Show } from "solid-js";
import { Dialog } from "./Dialog";

type Kind = "info" | "warning";

interface Choice {
  label: string;
  value: unknown;
  primary?: boolean;
}

interface Request {
  title: string;
  message: string;
  kind: Kind;
  choices: Choice[];
  resolve: (value: unknown) => void;
}

const [queue, setQueue] = createSignal<Request[]>([]);

export function choose<T>(title: string, message: string, choices: { label: string; value: T; primary?: boolean }[], kind: Kind = "info"): Promise<T | null> {
  return new Promise((resolve) => {
    setQueue((q) => [...q, { title, message, kind, choices, resolve: resolve as (value: unknown) => void }]);
  });
}

export async function ask(
  message: string,
  opts: { title: string; kind?: Kind; okLabel?: string; cancelLabel?: string },
): Promise<boolean> {
  const answer = await choose(
    opts.title,
    message,
    [
      { label: opts.cancelLabel ?? "Cancel", value: false },
      { label: opts.okLabel ?? "OK", value: true, primary: true },
    ],
    opts.kind,
  );
  return answer === true;
}

export async function message(text: string, opts: { title: string; kind?: Kind }): Promise<void> {
  await choose(opts.title, text, [{ label: "OK", value: true, primary: true }], opts.kind);
}

export function PromptHost() {
  const current = () => queue()[0];
  const settle = (value: unknown) => {
    const request = current();
    if (!request) return;
    setQueue((q) => q.slice(1));
    request.resolve(value);
  };
  return (
    <Show when={current()} keyed>
      {(request) => (
        <Dialog
          title={request.title}
          onClose={() => settle(null)}
          class="prompt"
          footer={
            <For each={request.choices}>
              {(choice) => {
                const destructive = request.kind === "warning" && choice.primary;
                return (
                  <button
                    class="btn"
                    classList={{ primary: choice.primary && !destructive, danger: destructive }}
                    autofocus={request.kind === "warning" ? !choice.primary : choice.primary}
                    onClick={() => settle(choice.value)}
                  >
                    {choice.label}
                  </button>
                );
              }}
            </For>
          }
        >
          <p class="prompt-message" classList={{ warning: request.kind === "warning" }}>
            {request.message}
          </p>
        </Dialog>
      )}
    </Show>
  );
}
