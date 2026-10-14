import { createSignal, onCleanup, onMount, Show } from "solid-js";
import { Portal } from "solid-js/web";

const SHOW_DELAY = 400;
const WARM_MS = 300;
const GAP = 6;
const EDGE = 8;
const TOOLTIP_ID = "app-tooltip";
const TARGETS = "[data-tooltip], .icon-button[aria-label], .window-control[aria-label]";

interface Tip {
  text: string;
  shortcut?: string;
}

function readTip(el: HTMLElement): Tip | null {
  const text = el.dataset.tooltip ?? el.getAttribute("aria-label");
  return text ? { text, shortcut: el.dataset.shortcut || undefined } : null;
}

export function TooltipHost() {
  let el!: HTMLDivElement;
  const [tip, setTip] = createSignal<Tip | null>(null);
  const [open, setOpen] = createSignal(false);
  let target: HTMLElement | null = null;
  let describes = false;
  let timer: number | undefined;
  let frame = 0;
  let closedAt = 0;

  const place = () => {
    if (!target?.isConnected) return hide();
    const rect = target.getBoundingClientRect();
    const { width, height } = el.getBoundingClientRect();
    const below = rect.bottom + GAP + height <= window.innerHeight - EDGE;
    const left = Math.min(Math.max(EDGE, rect.left + rect.width / 2 - width / 2), window.innerWidth - EDGE - width);
    el.style.left = `${left}px`;
    el.style.top = `${below ? rect.bottom + GAP : rect.top - GAP - height}px`;
    frame = requestAnimationFrame(place);
  };

  const hide = () => {
    clearTimeout(timer);
    cancelAnimationFrame(frame);
    if (describes) target?.removeAttribute("aria-describedby");
    describes = false;
    target = null;
    if (open()) closedAt = Date.now();
    setOpen(false);
  };

  const show = (next: HTMLElement) => {
    const content = readTip(next);
    hide();
    if (!content) return;
    target = next;
    setTip(content);
    if (!next.hasAttribute("aria-describedby")) {
      next.setAttribute("aria-describedby", TOOLTIP_ID);
      describes = true;
    }
    setOpen(true);
    place();
  };

  const schedule = (next: HTMLElement | null) => {
    if (next === target) return;
    if (!next) return hide();
    const warm = open() || Date.now() - closedAt < WARM_MS;
    hide();
    target = next;
    if (warm) show(next);
    else timer = window.setTimeout(() => show(next), SHOW_DELAY);
  };

  const closestTarget = (node: EventTarget | null) => (node instanceof Element ? node.closest<HTMLElement>(TARGETS) : null);
  const onOver = (e: PointerEvent) => schedule(closestTarget(e.target));
  const onOut = (e: PointerEvent) => !e.relatedTarget && hide();
  const onFocusIn = (e: FocusEvent) => {
    const next = closestTarget(e.target);
    if (next && next === e.target && next.matches(":focus-visible")) schedule(next);
    else hide();
  };

  onMount(() => {
    document.addEventListener("pointerover", onOver);
    document.addEventListener("pointerout", onOut);
    document.addEventListener("pointerdown", hide, true);
    document.addEventListener("keydown", hide, true);
    document.addEventListener("wheel", hide, { capture: true, passive: true });
    document.addEventListener("focusin", onFocusIn);
    window.addEventListener("blur", hide);
  });
  onCleanup(() => {
    hide();
    document.removeEventListener("pointerover", onOver);
    document.removeEventListener("pointerout", onOut);
    document.removeEventListener("pointerdown", hide, true);
    document.removeEventListener("keydown", hide, true);
    document.removeEventListener("wheel", hide, true);
    document.removeEventListener("focusin", onFocusIn);
    window.removeEventListener("blur", hide);
  });

  return (
    <Portal>
      <div ref={el} id={TOOLTIP_ID} class="tooltip" classList={{ open: open() }} role="tooltip">
        <span>{tip()?.text}</span>
        <Show when={tip()?.shortcut}>
          <span class="tooltip-shortcut">{tip()?.shortcut}</span>
        </Show>
      </div>
    </Portal>
  );
}
