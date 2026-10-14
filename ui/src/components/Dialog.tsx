import { createSignal, createUniqueId, onCleanup, onMount, type JSX, Show } from "solid-js";
import { CloseIcon } from "./Icons";

const open: symbol[] = [];
const [openCount, setOpenCount] = createSignal(0);

export const dialogOpen = () => openCount() > 0;

const FOCUSABLE = [
  "a[href]",
  "button:not(:disabled)",
  "input:not(:disabled)",
  "textarea:not(:disabled)",
  "select:not(:disabled)",
  '[tabindex]:not([tabindex="-1"])',
].join(", ");

function focusables(root: HTMLElement) {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.getClientRects().length > 0);
}

function initialFocus(root: HTMLElement) {
  for (const selector of ["[autofocus]", "input:not(:disabled), textarea:not(:disabled), select:not(:disabled)", "button.primary:not(:disabled)"]) {
    const el = root.querySelector<HTMLElement>(selector);
    if (el) return el;
  }
  return root;
}

export function Dialog(p: {
  title: string;
  onClose: () => void;
  footer?: JSX.Element;
  wide?: boolean;
  class?: string;
  children: JSX.Element;
}) {
  let ref!: HTMLDivElement;
  const self = Symbol();
  const titleId = createUniqueId();
  const previous = document.activeElement as HTMLElement | null;
  const topmost = () => open[open.length - 1] === self;

  const trapTab = (e: KeyboardEvent) => {
    const items = focusables(ref);
    if (!items.length) {
      e.preventDefault();
      ref.focus();
      return;
    }
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement as HTMLElement | null;
    const inside = !!active && ref.contains(active);
    if (e.shiftKey && (!inside || active === first)) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (!inside || active === last)) {
      e.preventDefault();
      first.focus();
    }
  };

  const onKey = (e: KeyboardEvent) => {
    if (!topmost()) return;
    if (e.key === "Escape") {
      if (document.querySelector(".menu, .select-list")) return;
      e.stopPropagation();
      p.onClose();
    } else if (e.key === "Tab" && !(e.target as HTMLElement).closest?.(".menu")) {
      trapTab(e);
    }
  };

  onMount(() => {
    open.push(self);
    setOpenCount(open.length);
    window.addEventListener("keydown", onKey, true);
    initialFocus(ref).focus();
  });
  onCleanup(() => {
    open.splice(open.indexOf(self), 1);
    setOpenCount(open.length);
    window.removeEventListener("keydown", onKey, true);
    if (previous?.isConnected) previous.focus();
  });

  return (
    <div class="backdrop" onMouseDown={(e) => e.target === e.currentTarget && p.onClose()}>
      <div
        ref={ref}
        class={`dialog ${p.wide ? "wide" : ""} ${p.class ?? ""}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
      >
        <header>
          <span id={titleId}>{p.title}</span>
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
