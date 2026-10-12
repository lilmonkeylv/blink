import { createVirtualizer } from "@tanstack/solid-virtual";
import { createEffect, For, on, type JSX } from "solid-js";

export function VirtualList<T>(p: {
  items: readonly T[];
  rowHeight: number;
  render: (item: T, index: number) => JSX.Element;
  class?: string;
  tabIndex?: number;
  onKeyDown?: (e: KeyboardEvent) => void;
  scrollToIndex?: number | null;
  onEndReached?: () => void;
  ref?: (el: HTMLDivElement) => void;
  style?: JSX.CSSProperties;
}) {
  let scroller!: HTMLDivElement;
  const virtualizer = createVirtualizer({
    get count() {
      return p.items.length;
    },
    getScrollElement: () => scroller,
    estimateSize: () => p.rowHeight,
    overscan: 12,
  });

  createEffect(
    on(
      () => p.scrollToIndex,
      (index) => {
        if (index != null && index >= 0) virtualizer.scrollToIndex(index, { align: "auto" });
      },
    ),
  );

  createEffect(() => {
    const items = virtualizer.getVirtualItems();
    const last = items[items.length - 1];
    if (last && p.onEndReached && last.index >= p.items.length - 20) p.onEndReached();
  });

  return (
    <div
      ref={(el) => {
        scroller = el;
        p.ref?.(el);
      }}
      class={`list ${p.class ?? ""}`}
      tabIndex={p.tabIndex ?? 0}
      onKeyDown={p.onKeyDown}
      style={p.style}
    >
      <div style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative", "min-width": "100%" }}>
        <For each={virtualizer.getVirtualItems()}>
          {(row) => (
            <div style={{ position: "absolute", top: `${row.start}px`, left: 0, right: 0, height: `${p.rowHeight}px` }}>
              {p.render(p.items[row.index], row.index)}
            </div>
          )}
        </For>
      </div>
    </div>
  );
}
