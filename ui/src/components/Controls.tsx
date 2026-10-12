import { createEffect, createMemo, createSignal, For, onCleanup, Show, type JSX } from "solid-js";
import { Portal } from "solid-js/web";
import { ChevronDown } from "./Icons";

export type CheckValue = boolean | "mixed";

export function Checkbox(p: {
  checked: CheckValue;
  onChange: (next: boolean) => void;
  label?: JSX.Element;
  ariaLabel?: string;
  disabled?: boolean;
  class?: string;
  stopPropagation?: boolean;
}) {
  const toggle = (e: Event) => {
    if (p.stopPropagation) e.stopPropagation();
    if (!p.disabled) p.onChange(p.checked !== true);
  };
  const box = (
    <span
      role="checkbox"
      tabIndex={p.label ? -1 : 0}
      aria-checked={p.checked === "mixed" ? "mixed" : p.checked}
      aria-label={p.ariaLabel}
      aria-disabled={p.disabled}
      class={`checkbox ${p.checked === true ? "on" : p.checked === "mixed" ? "mixed" : ""} ${p.disabled ? "disabled" : ""}`}
      onMouseDown={(e) => p.stopPropagation && e.stopPropagation()}
      onClick={p.label ? undefined : toggle}
      onKeyDown={(e) => {
        if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          toggle(e);
        }
      }}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <Show when={p.checked === "mixed"} fallback={<path d="M3.5 8.5 6.5 11.5 12.5 4.5" />}>
          <path d="M4 8h8" />
        </Show>
      </svg>
    </span>
  );
  return (
    <Show when={p.label} fallback={box}>
      <label
        class={`check ${p.class ?? ""}`}
        tabIndex={0}
        onClick={toggle}
        onKeyDown={(e) => {
          if (e.key === " " || e.key === "Enter") {
            e.preventDefault();
            toggle(e);
          }
        }}
      >
        {box}
        <span>{p.label}</span>
      </label>
    </Show>
  );
}

export interface SelectOption<T extends string> {
  value: T;
  label: string;
  hint?: string;
}

export function Select<T extends string>(p: {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  ariaLabel?: string;
  placeholder?: string;
  class?: string;
  disabled?: boolean;
}) {
  let button!: HTMLButtonElement;
  const [open, setOpen] = createSignal(false);
  const [active, setActive] = createSignal(0);
  const [rect, setRect] = createSignal<DOMRect | null>(null);
  const selected = createMemo(() => p.options.find((o) => o.value === p.value));

  const show = () => {
    if (p.disabled) return;
    setRect(button.getBoundingClientRect());
    setActive(Math.max(0, p.options.findIndex((o) => o.value === p.value)));
    setOpen(true);
  };
  const choose = (index: number) => {
    const option = p.options[index];
    if (option) p.onChange(option.value);
    setOpen(false);
    button.focus();
  };

  const onKey = (e: KeyboardEvent) => {
    if (!open()) {
      if (["ArrowDown", "ArrowUp", "Enter", " "].includes(e.key)) {
        e.preventDefault();
        show();
      }
      return;
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive(Math.min(active() + 1, p.options.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive(Math.max(active() - 1, 0));
    } else if (e.key === "Home") {
      setActive(0);
    } else if (e.key === "End") {
      setActive(p.options.length - 1);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose(active());
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.stopPropagation();
      setOpen(false);
    }
  };

  createEffect(() => {
    if (!open()) return;
    const close = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest(".select-list") && e.target !== button && !button.contains(e.target as Node)) setOpen(false);
    };
    const reposition = () => setOpen(false);
    window.addEventListener("mousedown", close);
    window.addEventListener("resize", reposition);
    window.addEventListener("blur", reposition);
    onCleanup(() => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("resize", reposition);
      window.removeEventListener("blur", reposition);
    });
  });

  const listStyle = () => {
    const r = rect();
    if (!r) return {};
    const height = Math.min(p.options.length * 30 + 10, 280);
    const below = window.innerHeight - r.bottom > height + 8;
    return {
      left: `${r.left}px`,
      width: `${r.width}px`,
      top: below ? `${r.bottom + 4}px` : `${Math.max(8, r.top - height - 4)}px`,
      "max-height": `${height}px`,
    };
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        class={`select ${open() ? "open" : ""} ${p.class ?? ""}`}
        aria-haspopup="listbox"
        aria-expanded={open()}
        aria-label={p.ariaLabel}
        disabled={p.disabled}
        onClick={() => (open() ? setOpen(false) : show())}
        onKeyDown={onKey}
      >
        <span class="select-value">{selected()?.label ?? p.placeholder ?? ""}</span>
        <ChevronDown />
      </button>
      <Show when={open()}>
        <Portal>
          <div class="select-list" role="listbox" style={listStyle()} onMouseDown={(e) => e.preventDefault()}>
            <For each={p.options}>
              {(option, i) => (
                <div
                  role="option"
                  aria-selected={option.value === p.value}
                  class="select-option"
                  classList={{ active: i() === active(), selected: option.value === p.value }}
                  onMouseEnter={() => setActive(i())}
                  onClick={() => choose(i())}
                >
                  <span class="grow">{option.label}</span>
                  <Show when={option.hint}>
                    <span class="muted">{option.hint}</span>
                  </Show>
                </div>
              )}
            </For>
          </div>
        </Portal>
      </Show>
    </>
  );
}

export function Field(p: { label: string; hint?: string; children: JSX.Element }) {
  return (
    <div class="field">
      <span class="field-label">{p.label}</span>
      {p.children}
      <Show when={p.hint}>
        <span class="hint">{p.hint}</span>
      </Show>
    </div>
  );
}

export function Segmented<T extends string>(p: { value: T; options: SelectOption<T>[]; onChange: (v: T) => void }) {
  return (
    <div class="segmented" role="radiogroup">
      <For each={p.options}>
        {(o) => (
          <button type="button" role="radio" aria-checked={o.value === p.value} classList={{ active: o.value === p.value }} onClick={() => p.onChange(o.value)}>
            {o.label}
          </button>
        )}
      </For>
    </div>
  );
}
