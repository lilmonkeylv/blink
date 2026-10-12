import { createMemo, createSignal, For, Match, onCleanup, Show, Switch } from "solid-js";
import type { DiffHunk, DiffLine, FileDiff } from "../api";
import { isLineIncluded, lineKey, type FileSel } from "../lib/selection";
import { openContextMenu, type MenuItem } from "./ContextMenu";
import { VirtualList } from "./VirtualList";

type Row =
  | { type: "hunk"; hunk: number; data: DiffHunk }
  | { type: "line"; hunk: number; line: number; data: DiffLine; key: string };

const ROW_HEIGHT = 20;

export function Tokens(p: { line: Pick<DiffLine, "text" | "tokens"> }) {
  const parts = () => {
    const tokens = p.line.tokens;
    const text = p.line.text;
    if (!tokens?.length) return [{ text, cls: "" }];
    const out: { text: string; cls: string }[] = [];
    let pos = 0;
    for (const [start, len, cls] of tokens) {
      if (start > pos) out.push({ text: text.slice(pos, start), cls: "" });
      out.push({ text: text.slice(start, start + len), cls: `t${cls}` });
      pos = start + len;
    }
    if (pos < text.length) out.push({ text: text.slice(pos), cls: "" });
    return out;
  };
  return <For each={parts()}>{(part) => (part.cls ? <span class={part.cls}>{part.text}</span> : part.text)}</For>;
}

function header(h: DiffHunk) {
  const range = (start: number, len: number) => (len === 1 ? `${start}` : `${start},${len}`);
  return `@@ -${range(h.oldStart, h.oldLen)} +${range(h.newStart, h.newLen)} @@`;
}

export function DiffView(p: {
  diff: FileDiff;
  selectable?: boolean;
  sel?: FileSel;
  onSetLines?: (keys: string[], include: boolean) => void;
  onDiscardLines?: (keys: string[]) => void;
  onLoadAnyway?: () => void;
}) {
  const rows = createMemo<Row[]>(() => {
    const out: Row[] = [];
    p.diff.hunks.forEach((h, hi) => {
      out.push({ type: "hunk", hunk: hi, data: h });
      h.lines.forEach((l, li) => out.push({ type: "line", hunk: hi, line: li, data: l, key: lineKey(hi, li) }));
    });
    return out;
  });

  const [cursor, setCursor] = createSignal<number | null>(null);
  const [drag, setDrag] = createSignal<{ from: number; to: number; include: boolean } | null>(null);

  const changedKeysBetween = (a: number, b: number) => {
    const [lo, hi] = a <= b ? [a, b] : [b, a];
    const keys: string[] = [];
    for (let i = lo; i <= hi; i++) {
      const r = rows()[i];
      if (r?.type === "line" && r.data.kind !== "context") keys.push(r.key);
    }
    return keys;
  };

  const hunkKeys = (hunk: number) =>
    rows()
      .filter((r): r is Extract<Row, { type: "line" }> => r.type === "line" && r.hunk === hunk && r.data.kind !== "context")
      .map((r) => r.key);

  const included = (key: string) => isLineIncluded(p.sel, key);

  const inDrag = (index: number) => {
    const d = drag();
    if (!d) return false;
    const [lo, hi] = d.from <= d.to ? [d.from, d.to] : [d.to, d.from];
    return index >= lo && index <= hi;
  };

  const lineIncludedView = (row: Extract<Row, { type: "line" }>, index: number) =>
    inDrag(index) ? drag()!.include : included(row.key);

  const endDrag = () => {
    const d = drag();
    if (!d) return;
    setDrag(null);
    p.onSetLines?.(changedKeysBetween(d.from, d.to), d.include);
  };
  window.addEventListener("mouseup", endDrag);
  onCleanup(() => window.removeEventListener("mouseup", endDrag));

  const startDrag = (index: number, row: Row) => {
    if (!p.selectable) return;
    if (row.type === "hunk") {
      const keys = hunkKeys(row.hunk);
      const allIn = keys.every(included);
      p.onSetLines?.(keys, !allIn);
      return;
    }
    if (row.data.kind === "context") return;
    setDrag({ from: index, to: index, include: !included(row.key) });
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const total = rows().length;
    if (!total) return;
    const cur = cursor() ?? -1;
    const move = (dir: 1 | -1) => {
      let i = cur;
      do i += dir;
      while (i >= 0 && i < total && !(rows()[i].type === "line" && (rows()[i] as Extract<Row, { type: "line" }>).data.kind !== "context"));
      if (i >= 0 && i < total) setCursor(i);
    };
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      move(1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      move(-1);
    } else if (e.key === " " && p.selectable && cur >= 0) {
      e.preventDefault();
      const row = rows()[cur];
      if (row.type === "line") p.onSetLines?.([row.key], !included(row.key));
    }
  };

  const contextMenu = (e: MouseEvent, index: number, row: Row) => {
    if (!p.selectable || !p.onDiscardLines) return;
    const items: MenuItem[] = [];
    if (row.type === "line" && row.data.kind !== "context") {
      items.push({ label: "Discard line…", run: () => p.onDiscardLines!(changedKeysBetween(index, index)) });
    }
    items.push({ label: "Discard hunk…", run: () => p.onDiscardLines!(hunkKeys(row.hunk)) });
    openContextMenu(e, items);
  };

  return (
    <Switch>
      <Match when={p.diff.binary}>
        <div class="empty">Binary file changed</div>
      </Match>
      <Match when={p.diff.tooLarge}>
        <div class="empty">
          <div>
            <h2>This diff is large</h2>
            <p>
              {p.diff.additions} additions, {p.diff.deletions} deletions.
            </p>
            <Show when={p.onLoadAnyway}>
              <button class="btn" onClick={() => p.onLoadAnyway?.()}>
                Load diff
              </button>
            </Show>
          </div>
        </div>
      </Match>
      <Match when={p.diff.hunks.length === 0}>
        <div class="empty">No content changes</div>
      </Match>
      <Match when={true}>
        <VirtualList
          class="diff"
          items={rows()}
          rowHeight={ROW_HEIGHT}
          onKeyDown={onKeyDown}
          scrollToIndex={cursor()}
          render={(row, index) =>
            row.type === "hunk" ? (
              <div class={`diff-row hunk ${p.selectable ? "selectable" : ""}`} onContextMenu={(e) => contextMenu(e, index, row)}>
                <div class="gutter" onMouseDown={() => startDrag(index, row)} title={p.selectable ? "Toggle hunk" : undefined}>
                  <span class="no" />
                  <span class="no" />
                </div>
                <span class="marker" />
                <span class="code">{header(row.data)}</span>
              </div>
            ) : (
              <div
                class={`diff-row ${row.data.kind}`}
                classList={{
                  selectable: !!p.selectable && row.data.kind !== "context",
                  included: !!p.selectable && row.data.kind !== "context" && lineIncludedView(row, index),
                  excluded: !!p.selectable && row.data.kind !== "context" && !lineIncludedView(row, index),
                  cursor: cursor() === index,
                }}
                onContextMenu={(e) => contextMenu(e, index, row)}
              >
                <div
                  class="gutter"
                  onMouseDown={(e) => {
                    e.preventDefault();
                    startDrag(index, row);
                  }}
                  onMouseEnter={() => drag() && setDrag({ ...drag()!, to: index })}
                >
                  <span class="no">{row.data.oldNo ?? ""}</span>
                  <span class="no">{row.data.newNo ?? ""}</span>
                </div>
                <span class="marker">{row.data.kind === "added" ? "+" : row.data.kind === "removed" ? "-" : " "}</span>
                <span class="code">
                  <Tokens line={row.data} />
                  <Show when={row.data.noNewline}>
                    <span class="eol">No newline at end of file</span>
                  </Show>
                </span>
              </div>
            )
          }
        />
      </Match>
    </Switch>
  );
}
