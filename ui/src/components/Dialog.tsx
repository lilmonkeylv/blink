import { onCleanup, onMount, type JSX, Show } from "solid-js";
import { CloseIcon } from "./Icons";

export function Dialog(p: {
  title: string;
  onClose: () => void;
  footer?: JSX.Element;
  wide?: boolean;
  class?: string;
  children: JSX.Element;
}) {
  let ref!: HTMLDivElement;
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      p.onClose();
    }
  };
  onMount(() => {
    window.addEventListener("keydown", onKey, true);
    ref.querySelector<HTMLElement>("[autofocus], input, textarea, select, button.primary")?.focus();
  });
  onCleanup(() => window.removeEventListener("keydown", onKey, true));

  return (
    <div class="backdrop" onMouseDown={(e) => e.target === e.currentTarget && p.onClose()}>
      <div ref={ref} class={`dialog ${p.wide ? "wide" : ""} ${p.class ?? ""}`} role="dialog" aria-label={p.title}>
        <header>
          <span>{p.title}</span>
          <button class="icon-button" onClick={p.onClose} aria-label="Close">
            <CloseIcon />
          </button>
        </header>
        <div class="body">{p.children}</div>
        <Show when={p.footer}>
          <footer>{p.footer}</footer>
        </Show>
      </div>
    </div>
  );
}
