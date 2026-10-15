import { createVirtualizer } from "@tanstack/solid-virtual";
import { createEffect, createMemo, For, on, Show, type JSX } from "solid-js";

export function VirtualList<T>(p: {
  items: readonly T[];
  rowHeight: number;
  render: (item: T, index: number) => JSX.Element;
  class?: string;
  tabIndex?: number;
  onKeyDown?: (e: KeyboardEvent) => void;
  scrollToIndex?: number | null;
  onEndReached?: () => void;
  onRangeChange?: (start: number, end: number) => void;
  ref?: (el: HTMLDivElement) => void;
  style?: JSX.CSSProperties;
  dynamic?: boolean;
  itemKey?: (item: T, index: number) => string | number;
}) {
  let scroller!: HTMLDivElement;
  const virtualizer = createVirtualizer({
    get count() {
      return p.items.length;
    },
    getScrollElement: () => scroller,
    estimateSize: () => p.rowHeight,
    get getItemKey() {
      const key = p.itemKey;
      return key ? (index: number) => key(p.items[index], index) : (index: number) => index;
    },
    overscan: 12,
  });

  const indexByKey = createMemo(() => new Map(virtualizer.getVirtualItems().map((v) => [v.key, v.index])));
  const measure = (el: HTMLDivElement) => queueMicrotask(() => el.isConnected && virtualizer.measureElement(el));

  createEffect(
    on(
      () => (p.itemKey && p.items.length ? p.itemKey(p.items[0], 0) : undefined),
      (first, previous) => {
        if (previous !== undefined && first !== previous) virtualizer.scrollToOffset(0);
      },
    ),
  );

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
    if (items.length) p.onRangeChange?.(items[0].index, last.index);
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
        <Show
          when={p.dynamic}
          fallback={
            <For each={virtualizer.getVirtualItems()}>
              {(row) => (
                <div style={{ position: "absolute", top: `${row.start}px`, left: 0, right: 0, height: `${p.rowHeight}px` }}>
                  {p.render(p.items[row.index], row.index)}
                </div>
              )}
            </For>
          }
        >
          <div
            style={{
              position: "absolute",
              top: 0,
              left: 0,
              right: 0,
              transform: `translateY(${virtualizer.getVirtualItems()[0]?.start ?? 0}px)`,
            }}
          >
            <Show
              when={p.itemKey}
              fallback={
                <For each={virtualizer.getVirtualItems()}>
                  {(row) => (
                    <div data-index={row.index} ref={measure}>
                      {p.render(p.items[row.index], row.index)}
                    </div>
                  )}
                </For>
              }
            >
              <For each={[...indexByKey().keys()]}>
                {(key) => {
                  const index = indexByKey().get(key) ?? 0;
                  return (
                    <div data-index={indexByKey().get(key)} ref={measure}>
                      {p.render(p.items[index], index)}
                    </div>
                  );
                }}
              </For>
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}
