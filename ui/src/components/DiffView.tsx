import { createMemo, createSignal, For, Match, onCleanup, Show, Switch } from "solid-js";
import type { DiffHunk, DiffLine, FileDiff, LfsDiff, LfsPointer, SubmoduleDiff } from "../api";
import { isLineIncluded, lineKey, type FileSel } from "../lib/selection";
import { formatBytes } from "../lib/size";
import { pairLines, type Cell } from "../lib/split";
import { openContextMenu, type MenuItem } from "./ContextMenu";
import { Segmented } from "./Controls";
import { VirtualList } from "./VirtualList";

type Row =
  | { type: "hunk"; hunk: number; data: DiffHunk }
  | { type: "line"; hunk: number; line: number; data: DiffLine; key: string };

type Side = "left" | "right";

const ROW_HEIGHT = 20;

function readLayout(): "unified" | "split" {
  try {
    return localStorage.getItem("blink.diffLayout") === "split" ? "split" : "unified";
  } catch {
    return "unified";
  }
}

const [diffLayout, setLayoutSignal] = createSignal<"unified" | "split">(readLayout());

export { diffLayout };

export function setDiffLayout(layout: "unified" | "split") {
  setLayoutSignal(layout);
  try {
    localStorage.setItem("blink.diffLayout", layout);
  } catch {}
}

export function DiffLayoutToggle() {
  return (
    <Segmented
      value={diffLayout()}
      onChange={setDiffLayout}
      options={[
        { value: "unified", label: "Unified" },
        { value: "split", label: "Split" },
      ]}
    />
  );
}


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
  onStashLines?: (keys: string[]) => void;
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

  const splitRows = createMemo(() => pairLines(p.diff.hunks));
  const [splitDrag, setSplitDrag] = createSignal<{ side: Side; from: number; to: number; include: boolean } | null>(null);

  const sideKeysBetween = (side: Side, a: number, b: number) => {
    const [lo, hi] = a <= b ? [a, b] : [b, a];
    const keys: string[] = [];
    for (let i = lo; i <= hi; i++) {
      const r = splitRows()[i];
      const cell = r?.type === "pair" ? r[side] : undefined;
      if (cell && cell.data.kind !== "context") keys.push(cell.key);
    }
    return keys;
  };

  const cellIncludedView = (side: Side, index: number, cell: Cell) => {
    const d = splitDrag();
    if (d && d.side === side) {
      const [lo, hi] = d.from <= d.to ? [d.from, d.to] : [d.to, d.from];
      if (index >= lo && index <= hi) return d.include;
    }
    return included(cell.key);
  };

  const endDrag = () => {
    const sd = splitDrag();
    if (sd) {
      setSplitDrag(null);
      p.onSetLines?.(sideKeysBetween(sd.side, sd.from, sd.to), sd.include);
    }
    const d = drag();
    if (!d) return;
    setDrag(null);
    p.onSetLines?.(changedKeysBetween(d.from, d.to), d.include);
  };

  const Half = (h: { side: Side; cell?: Cell; index: number; hunk: number }) => {
    const changed = () => !!h.cell && h.cell.data.kind !== "context";
    const kind = () => (!h.cell ? "filler" : h.cell.data.kind);
    return (
      <div
        class={`diff-row split-half ${kind()}`}
        classList={{
          selectable: !!p.selectable && changed(),
          included: !!p.selectable && changed() && cellIncludedView(h.side, h.index, h.cell!),
          excluded: !!p.selectable && changed() && !cellIncludedView(h.side, h.index, h.cell!),
        }}
        onContextMenu={(e) => {
          if (!p.selectable || !p.onDiscardLines) return;
          const items: MenuItem[] = [];
          if (changed()) items.push({ label: "Discard line…", run: () => p.onDiscardLines!([h.cell!.key]) });
          items.push({ label: "Discard hunk…", run: () => p.onDiscardLines!(hunkKeys(h.hunk)) });
          if (p.onStashLines) {
            items.push("separator");
            if (changed()) items.push({ label: "Stash line", run: () => p.onStashLines!([h.cell!.key]) });
            items.push({ label: "Stash hunk", run: () => p.onStashLines!(hunkKeys(h.hunk)) });
          }
          openContextMenu(e, items);
        }}
      >
        <div
          class="gutter"
          onMouseDown={(e) => {
            e.preventDefault();
            if (p.selectable && changed()) setSplitDrag({ side: h.side, from: h.index, to: h.index, include: !included(h.cell!.key) });
          }}
          onMouseEnter={() => splitDrag()?.side === h.side && setSplitDrag({ ...splitDrag()!, to: h.index })}
        >
          <span class="no">{h.cell ? (h.side === "left" ? h.cell.data.oldNo : h.cell.data.newNo) ?? "" : ""}</span>
        </div>
        <span class="marker">{kind() === "added" ? "+" : kind() === "removed" ? "-" : " "}</span>
        <span class="code">
          <Show when={h.cell}>
            {(c) => (
              <>
                <Tokens line={c().data} />
                <Show when={c().data.noNewline}>
                  <span class="eol">No newline at end of file</span>
                </Show>
              </>
            )}
          </Show>
        </span>
      </div>
    );
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
    if (p.onStashLines) {
      items.push("separator");
      if (row.type === "line" && row.data.kind !== "context") {
        items.push({ label: "Stash line", run: () => p.onStashLines!(changedKeysBetween(index, index)) });
      }
      items.push({ label: "Stash hunk", run: () => p.onStashLines!(hunkKeys(row.hunk)) });
    }
    openContextMenu(e, items);
  };

  return (
    <Switch>
      <Match when={p.diff.submodule}>{(s) => <SubmodulePanel diff={s()} />}</Match>
      <Match when={p.diff.lfs}>{(l) => <LfsPanel diff={l()} />}</Match>
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
      <Match when={diffLayout() === "split"}>
        <VirtualList
          class="diff"
          items={splitRows()}
          rowHeight={ROW_HEIGHT}
          dynamic
          render={(row, index) =>
            row.type === "hunk" ? (
              <div class={`diff-row hunk ${p.selectable ? "selectable" : ""}`}>
                <div
                  class="gutter"
                  onMouseDown={() => {
                    if (!p.selectable) return;
                    const keys = hunkKeys(row.hunk);
                    p.onSetLines?.(keys, !keys.every(included));
                  }}
                  title={p.selectable ? "Toggle hunk" : undefined}
                >
                  <span class="no" />
                </div>
                <span class="marker" />
                <span class="code">{header(row.data)}</span>
              </div>
            ) : (
              <div class="split-row">
                <Half side="left" cell={row.left} index={index} hunk={row.hunk} />
                <Half side="right" cell={row.right} index={index} hunk={row.hunk} />
              </div>
            )
          }
        />
      </Match>
      <Match when={true}>
        <VirtualList
          class="diff"
          items={rows()}
          rowHeight={ROW_HEIGHT}
          dynamic
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

const short = (sha: string | null) => sha?.slice(0, 7) ?? "";

function SubmodulePanel(p: { diff: SubmoduleDiff }) {
  const title = () => {
    if (!p.diff.old) return "Submodule added";
    if (!p.diff.new) return "Submodule removed";
    if (p.diff.old === p.diff.new) return "Submodule has local changes";
    return "Submodule updated";
  };
  return (
    <div class="empty">
      <div class="special-diff">
        <h2>{title()}</h2>
        <Show when={p.diff.old && p.diff.new && p.diff.old !== p.diff.new}>
          <p class="mono">
            {short(p.diff.old)} → {short(p.diff.new)}
          </p>
        </Show>
        <Show when={!p.diff.old || !p.diff.new}>
          <p class="mono">{short(p.diff.old ?? p.diff.new)}</p>
        </Show>
        <Show when={p.diff.dirty}>
          <p>The submodule's working tree has uncommitted changes.</p>
        </Show>
        <Show when={p.diff.commits.length > 0}>
          <ul class="special-list">
            <For each={p.diff.commits}>
              {([sha, summary]) => (
                <li>
                  <span class="mono">{sha}</span> {summary}
                </li>
              )}
            </For>
          </ul>
        </Show>
        <Show when={p.diff.old && p.diff.new && p.diff.old !== p.diff.new && p.diff.commits.length === 0}>
          <p>Moved to a commit that isn't ahead of the old one, or isn't checked out locally.</p>
        </Show>
      </div>
    </div>
  );
}

function LfsSide(p: { label: string; pointer: LfsPointer | null }) {
  return (
    <Show when={p.pointer}>
      {(ptr) => (
        <p>
          {p.label}: {formatBytes(ptr().size)} <span class="mono">{ptr().oid.slice(0, 12)}</span>
        </p>
      )}
    </Show>
  );
}

function LfsPanel(p: { diff: LfsDiff }) {
  const title = () => {
    if (!p.diff.old) return "Git LFS file added";
    if (!p.diff.new) return "Git LFS file removed";
    if (p.diff.old.oid === p.diff.new.oid) return "Git LFS file unchanged";
    return "Git LFS file changed";
  };
  return (
    <div class="empty">
      <div class="special-diff">
        <h2>{title()}</h2>
        <LfsSide label="Before" pointer={p.diff.old} />
        <LfsSide label="After" pointer={p.diff.new} />
      </div>
    </div>
  );
}
