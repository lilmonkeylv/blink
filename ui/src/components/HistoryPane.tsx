import { ask } from "@tauri-apps/plugin-dialog";
import { createEffect, createMemo, createResource, createSignal, For, on, Show } from "solid-js";
import { api, type CommitFile, type CommitSummary, type FileDiff, type GraphRow, type SearchKind } from "../api";
import { absoluteTime, relativeTime } from "../lib/time";
import { currentBranch, notify, reportError, run, setStore, store } from "../store";
import { openContextMenu, type MenuItem } from "./ContextMenu";
import { Checkbox, Select, Segmented } from "./Controls";
import { DiffView } from "./DiffView";
import { CloseIcon, StatusIcon } from "./Icons";
import { ImageDiff, isImagePath } from "./ImageDiff";
import { VirtualList } from "./VirtualList";

const PAGE = 300;
const ROW = 52;
const LANE = 12;
const LANE_COLORS = ["#2f81f7", "#3fb950", "#d29922", "#db61a2", "#a371f7", "#f85149", "#39c5cf", "#e3b341"];
export const DRAG_TYPE = "application/x-blink-commits";

const [commits, setCommits] = createSignal<CommitSummary[]>([]);
const [rows, setRows] = createSignal<GraphRow[]>([]);
const [selected, setSelected] = createSignal<string[]>([]);
const [anchor, setAnchor] = createSignal<string | null>(null);
const [compareSide, setCompareSide] = createSignal<"behind" | "ahead">("behind");
const [compareResult, setCompareResult] = createSignal<{ ahead: CommitSummary[]; behind: CommitSummary[] } | null>(null);
const [showGraph, setShowGraph] = createSignal(readGraphPref());
let loading = false;
let exhausted = false;
let loadedFor = "";

function readGraphPref() {
  try {
    return localStorage.getItem("blink.graph") !== "off";
  } catch {
    return true;
  }
}

function setGraphPref(on: boolean) {
  setShowGraph(on);
  try {
    localStorage.setItem("blink.graph", on ? "on" : "off");
  } catch {}
}

function headId() {
  const head = store.repo?.head;
  return head && head.type !== "unborn" ? head.id : "";
}

async function loadMore(reset = false) {
  const repo = store.repo;
  if (!repo || loading || (exhausted && !reset)) return;
  loading = true;
  try {
    const offset = reset ? 0 : commits().length;
    const page = await api.historyGraph(repo.id, offset, PAGE);
    exhausted = page.commits.length < PAGE;
    setCommits(reset ? page.commits : [...commits(), ...page.commits]);
    setRows(reset ? page.rows : [...rows(), ...page.rows]);
    if (reset && !page.commits.some((c) => selected().includes(c.id))) setSelected(page.commits[0] ? [page.commits[0].id] : []);
  } catch (e) {
    reportError(e, "Could not load history");
  } finally {
    loading = false;
  }
}

export function reloadHistory() {
  loadedFor = "";
  exhausted = false;
  loadMore(true);
}

function Graph(p: { row: GraphRow; width: number }) {
  const x = (lane: number) => 8 + lane * LANE;
  const color = (c: number) => LANE_COLORS[c % LANE_COLORS.length];
  const path = (from: number, to: number, y0: number, y1: number) =>
    from === to ? `M${x(from)} ${y0}V${y1}` : `M${x(from)} ${y0}C${x(from)} ${(y0 + y1) / 2} ${x(to)} ${(y0 + y1) / 2} ${x(to)} ${y1}`;
  return (
    <svg class="graph" width={p.width} height={ROW} aria-hidden="true">
      <For each={p.row.top}>{(e) => <path d={path(e.from, e.to, 0, ROW / 2)} stroke={color(e.color)} />}</For>
      <For each={p.row.bottom}>{(e) => <path d={path(e.from, e.to, ROW / 2, ROW)} stroke={color(e.color)} />}</For>
      <circle cx={x(p.row.lane)} cy={ROW / 2} r="4" fill="var(--bg)" stroke={color(p.row.color)} stroke-width="2.5" />
    </svg>
  );
}

function isMerge(c: CommitSummary) {
  return c.parents.length > 1;
}

export function commitMenu(c: CommitSummary, list: CommitSummary[]): MenuItem[] {
  const repo = store.repo!;
  const multi = selected().length > 1 && selected().includes(c.id);
  if (multi) {
    const shas = list.filter((x) => selected().includes(x.id)).map((x) => x.id);
    const oldest = shas[shas.length - 1];
    return [
      { label: `Cherry-pick ${shas.length} commits…`, run: () => setStore("dialog", { type: "cherryPick", shas: [...shas].reverse() }) },
      { label: `Squash ${shas.length} commits…`, run: () => setStore("dialog", { type: "squash", target: oldest, commits: shas.slice(0, -1) }) },
      "separator",
      { label: "Copy SHAs", run: () => navigator.clipboard.writeText(shas.join("\n")) },
    ];
  }
  const isHead = c.id === headId();
  return [
    { label: "Reset to commit…", run: () => setStore("dialog", { type: "reset", sha: c.id }), disabled: isHead },
    { label: "Check out commit", run: () => checkoutDetached(c) },
    { label: "Revert changes in commit", run: () => run("Revert", () => api.revert(repo.id, c.id)) },
    "separator",
    { label: "Create branch from commit…", run: () => setStore("dialog", { type: "newBranch", start: c.id }) },
    { label: "Create tag…", run: () => setStore("dialog", { type: "tag", sha: c.id }) },
    { label: "Cherry-pick commit…", run: () => setStore("dialog", { type: "cherryPick", shas: [c.id] }) },
    { label: "Edit history from here…", run: () => setStore("dialog", { type: "interactiveRebase", sha: c.id }), disabled: isMerge(c) },
    "separator",
    { label: "Copy SHA", run: () => navigator.clipboard.writeText(c.id) },
    { label: "Copy summary", run: () => navigator.clipboard.writeText(c.summary) },
    { label: "Copy as patch", run: () => api.commitPatch(repo.id, c.id).then((p) => navigator.clipboard.writeText(p)).then(() => notify("info", "Patch copied")).catch(reportError) },
  ];
}

async function checkoutDetached(c: CommitSummary) {
  const repo = store.repo!;
  const ok = await ask(`Check out ${c.shortId}? You will be in a detached HEAD state.`, { title: "Check out commit", okLabel: "Check out" });
  if (ok) await run("Check out", () => api.checkoutCommit(repo.id, c.id));
}

async function reorder(dragged: string[], target: string, after: boolean) {
  const repo = store.repo!;
  const list = commits();
  const indices = [...dragged.map((d) => list.findIndex((c) => c.id === d)), list.findIndex((c) => c.id === target)];
  if (indices.some((i) => i < 0)) return;
  const oldest = Math.max(...indices);
  const range = list.slice(0, oldest + 1);
  if (range.some(isMerge)) return notify("error", "Reordering across merge commits is not supported.");
  const moving = range.filter((c) => dragged.includes(c.id));
  const rest = range.filter((c) => !dragged.includes(c.id));
  const at = rest.findIndex((c) => c.id === target) + (after ? 1 : 0);
  const newestFirst = [...rest.slice(0, at), ...moving, ...rest.slice(at)];
  if (newestFirst.every((c, i) => c.id === range[i].id)) return;
  const base = range[range.length - 1].parents[0] ?? null;
  await run("Reorder commits", () =>
    api.rebaseInteractive(repo.id, {
      base,
      label: "Reorder commits",
      steps: [...newestFirst].reverse().map((c) => ({ action: "pick", sha: c.id })),
    }),
  );
}

function SearchBar() {
  const [kind, setKind] = createSignal<SearchKind>(store.historySearch?.kind ?? "message");
  const [query, setQuery] = createSignal(store.historySearch?.query ?? "");
  let timer: number | undefined;
  const apply = () => {
    clearTimeout(timer);
    timer = window.setTimeout(() => setStore("historySearch", query().trim() ? { kind: kind(), query: query().trim() } : null), 250);
  };
  return (
    <div class="history-tools">
      <input
        id="history-search"
        class="input"
        placeholder="Search history"
        value={query()}
        onInput={(e) => {
          setQuery(e.currentTarget.value);
          apply();
        }}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            setQuery("");
            setStore("historySearch", null);
          }
        }}
      />
      <div style={{ width: "112px", flex: "none" }}>
        <Select
          value={kind()}
          onChange={(k) => {
            setKind(k);
            apply();
          }}
          ariaLabel="Search in"
          options={[
            { value: "message", label: "Message" },
            { value: "author", label: "Author" },
            { value: "content", label: "Content" },
            { value: "path", label: "Path" },
          ]}
        />
      </div>
    </div>
  );
}

function CompareBar() {
  const [branches] = createResource(() => store.repo?.id, (id) => api.branches(id));
  const options = createMemo(() => [
    { value: "", label: "Compare to branch…" },
    ...(branches() ?? []).filter((b) => b.name !== currentBranch()).map((b) => ({ value: b.name, label: b.name })),
  ]);
  createEffect(
    on(
      () => [store.compareWith, store.repo?.generation] as const,
      async ([other]) => {
        if (!other || !store.repo) return setCompareResult(null);
        try {
          setCompareResult(await api.compare(store.repo.id, other));
        } catch (e) {
          reportError(e, "Compare failed");
        }
      },
    ),
  );
  return (
    <div class="history-tools">
      <div style={{ flex: 1, "min-width": 0 }}>
        <Select value={store.compareWith ?? ""} onChange={(v) => setStore("compareWith", v || null)} options={options()} ariaLabel="Compare branch" />
      </div>
      <Show when={store.compareWith}>
        <button class="icon-button" aria-label="Stop comparing" onClick={() => setStore("compareWith", null)}>
          <CloseIcon />
        </button>
      </Show>
    </div>
  );
}

export function HistoryList() {
  createEffect(
    on(
      () => [store.repo?.id, headId()] as const,
      ([id, head]) => {
        const key = `${id}@${head}`;
        if (!id || key === loadedFor) return;
        loadedFor = key;
        exhausted = false;
        loadMore(true);
      },
    ),
  );

  const [searchResults] = createResource(
    () => (store.repo && store.historySearch ? { id: store.repo.id, ...store.historySearch } : null),
    ({ id, kind, query }) => api.searchHistory(id, kind, query),
  );

  const mode = () => (store.historySearch ? "search" : store.compareWith ? "compare" : "log");
  const list = createMemo<CommitSummary[]>(() => {
    if (mode() === "search") return searchResults() ?? [];
    if (mode() === "compare") return compareResult()?.[compareSide()] ?? [];
    return commits();
  });
  const graphOn = () => mode() === "log" && showGraph();
  const graphWidth = createMemo(() => (graphOn() ? Math.min(8, Math.max(1, ...rows().slice(0, 400).map((r) => r.width))) * LANE + 8 : 0));

  const primary = () => selected()[0] ?? null;
  const index = () => list().findIndex((c) => c.id === primary());
  const dateFormat = () => store.settings?.appearance.dateFormat ?? "relative";

  const click = (e: MouseEvent, c: CommitSummary) => {
    if (e.button !== 0 && selected().includes(c.id)) return;
    if (e.shiftKey && anchor()) {
      const items = list();
      const a = items.findIndex((x) => x.id === anchor());
      const b = items.findIndex((x) => x.id === c.id);
      const [lo, hi] = a < b ? [a, b] : [b, a];
      setSelected(items.slice(lo, hi + 1).map((x) => x.id));
    } else if (e.ctrlKey || e.metaKey) {
      setSelected(selected().includes(c.id) ? selected().filter((s) => s !== c.id) : [...selected(), c.id]);
      setAnchor(c.id);
    } else {
      setSelected([c.id]);
      setAnchor(c.id);
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const i = index();
    const items = list();
    if ((e.key === "ArrowDown" || e.key === "j") && i < items.length - 1) {
      e.preventDefault();
      setSelected([items[i + 1].id]);
      setAnchor(items[i + 1].id);
    } else if ((e.key === "ArrowUp" || e.key === "k") && i > 0) {
      e.preventDefault();
      setSelected([items[i - 1].id]);
      setAnchor(items[i - 1].id);
    }
  };

  const [dropHint, setDropHint] = createSignal<{ id: string; zone: "before" | "on" | "after" } | null>(null);
  const zone = (e: DragEvent): "before" | "on" | "after" => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const y = e.clientY - r.top;
    return y < r.height * 0.25 ? "before" : y > r.height * 0.75 ? "after" : "on";
  };
  const onDrop = (e: DragEvent, target: CommitSummary) => {
    e.preventDefault();
    setDropHint(null);
    const data = e.dataTransfer?.getData(DRAG_TYPE);
    if (!data) return;
    const dragged: string[] = JSON.parse(data);
    if (dragged.includes(target.id)) return;
    const z = zone(e);
    if (z === "on") setStore("dialog", { type: "squash", target: target.id, commits: dragged });
    else reorder(dragged, target.id, z === "after");
  };

  return (
    <>
      <SearchBar />
      <Show when={!store.historySearch}>
        <CompareBar />
      </Show>
      <Show when={mode() === "compare" && compareResult()}>
        {(r) => (
          <div class="history-tools" style={{ "justify-content": "space-between" }}>
            <Segmented
              value={compareSide()}
              onChange={setCompareSide}
              options={[
                { value: "behind", label: `Behind (${r().behind.length})` },
                { value: "ahead", label: `Ahead (${r().ahead.length})` },
              ]}
            />
            <Show when={compareSide() === "behind" && r().behind.length}>
              <button
                class="btn"
                onClick={() =>
                  run("Merge", () => api.merge(store.repo!.id, store.compareWith!)).then((res) => {
                    if (res?.conflicted) notify("error", "Merge has conflicts. Resolve them in the Changes tab.");
                  })
                }
              >
                Merge into {currentBranch()}
              </button>
            </Show>
          </div>
        )}
      </Show>
      <Show when={mode() === "log"}>
        <div class="list-header" style={{ "justify-content": "flex-end" }}>
          <Checkbox checked={showGraph()} onChange={setGraphPref} label="Graph" class="small" />
        </div>
      </Show>
      <Show
        when={list().length}
        fallback={<div class="empty">{mode() === "search" ? (searchResults.loading ? "Searching…" : "No matching commits") : "No commits"}</div>}
      >
        <VirtualList
          items={list()}
          rowHeight={ROW}
          onKeyDown={onKeyDown}
          scrollToIndex={index()}
          onEndReached={() => mode() === "log" && loadMore()}
          render={(c, i) => (
            <div
              class="row history-row"
              classList={{
                selected: selected().includes(c.id),
                "drop-before": dropHint()?.id === c.id && dropHint()?.zone === "before",
                "drop-on": dropHint()?.id === c.id && dropHint()?.zone === "on",
                "drop-after": dropHint()?.id === c.id && dropHint()?.zone === "after",
              }}
              draggable={mode() === "log"}
              onDragStart={(e) => {
                const shas = selected().includes(c.id) ? list().filter((x) => selected().includes(x.id)).map((x) => x.id) : [c.id];
                e.dataTransfer?.setData(DRAG_TYPE, JSON.stringify(shas));
                e.dataTransfer!.effectAllowed = "copyMove";
              }}
              onDragOver={(e) => {
                if (!e.dataTransfer?.types.includes(DRAG_TYPE)) return;
                e.preventDefault();
                setDropHint({ id: c.id, zone: zone(e) });
              }}
              onDragLeave={() => setDropHint(null)}
              onDrop={(e) => onDrop(e, c)}
              onMouseDown={(e) => click(e, c)}
              onContextMenu={(e) => openContextMenu(e, commitMenu(c, list()))}
              title={c.summary}
            >
              <Show when={graphOn() && rows()[i]}>
                <Graph row={rows()[i]} width={graphWidth()} />
              </Show>
              <div class="history-text">
                <span class="summary">{c.summary || "(no message)"}</span>
                <span class="meta">
                  {c.authorName} · {dateFormat() === "relative" ? relativeTime(c.time) : absoluteTime(c.time)} · {c.shortId}
                </span>
              </div>
            </div>
          )}
        />
      </Show>
    </>
  );
}

function FileList(p: { files: CommitFile[]; current: CommitFile | null; onSelect: (f: CommitFile) => void; sha?: string }) {
  return (
    <VirtualList
      items={p.files}
      rowHeight={30}
      render={(f) => (
        <div
          class="row"
          classList={{ selected: f.path === p.current?.path }}
          onMouseDown={() => p.onSelect(f)}
          onContextMenu={(e) =>
            openContextMenu(e, [
              { label: "Copy file path", run: () => navigator.clipboard.writeText(f.path) },
              { label: "File history", run: () => setStore("dialog", { type: "fileHistory", path: f.path }) },
              { label: "Blame", run: () => setStore("dialog", { type: "blame", path: f.path }), disabled: f.kind === "deleted" },
            ])
          }
          title={f.path}
        >
          <span class="path">
            <bdi>{f.path}</bdi>
          </span>
          <StatusIcon kind={f.kind} />
        </div>
      )}
    />
  );
}

export function CommitView() {
  const range = createMemo(() => {
    const ids = selected();
    if (ids.length < 2) return null;
    const ordered = commits().filter((c) => ids.includes(c.id));
    return ordered.length >= 2 ? { newest: ordered[0].id, oldest: ordered[ordered.length - 1].id, count: ordered.length } : null;
  });

  const [detail] = createResource(
    () => (store.repo && selected()[0] && !range() ? { id: store.repo.id, sha: selected()[0] } : null),
    ({ id, sha }) => api.commitDetail(id, sha),
  );
  const [rangeFiles] = createResource(
    () => (store.repo && range() ? { id: store.repo.id, ...range()! } : null),
    ({ id, oldest, newest }) => api.rangeFiles(id, oldest, newest),
  );
  const files = () => (range() ? rangeFiles() ?? [] : detail()?.files ?? []);
  const [file, setFile] = createSignal<CommitFile | null>(null);
  createEffect(on(files, (f) => setFile(f[0] ?? null)));

  const [diff, setDiff] = createSignal<FileDiff | null>(null);
  let diffToken = 0;
  createEffect(
    on(
      () => {
        const f = file();
        if (!store.repo || !f || isImagePath(f.path)) return null;
        return { id: store.repo.id, f, range: range(), sha: selected()[0] };
      },
      async (target) => {
        const mine = ++diffToken;
        setDiff(null);
        if (!target) return;
        const { id, f, range, sha } = target;
        const load = (highlight: boolean) =>
          range
            ? api.rangeFileDiff(id, range.oldest, range.newest, f.path, f.oldPath, highlight)
            : api.commitFileDiff(id, sha, f.path, f.oldPath, false, highlight);
        try {
          const plain = await load(false);
          if (mine !== diffToken) return;
          setDiff(plain);
          const colored = await load(true);
          if (mine === diffToken) setDiff(colored);
        } catch (e) {
          if (mine === diffToken) reportError(e);
        }
      },
    ),
  );

  const body = () => (
    <div class="commit-split">
      <FileList files={files()} current={file()} onSelect={setFile} />
      <div class="content">
        <Show when={file()} fallback={<div class="empty muted">No files</div>}>
          {(f) => (
            <Show
              when={!isImagePath(f().path)}
              fallback={<ImageDiff path={f().path} oldPath={f().oldPath} sha={range() ? undefined : selected()[0]} />}
            >
              <Show when={diff()} fallback={<div class="empty muted">Loading…</div>}>
                {(d) => <DiffView diff={d()} />}
              </Show>
            </Show>
          )}
        </Show>
      </div>
    </div>
  );

  return (
    <Show
      when={range()}
      fallback={
        <Show when={detail()} fallback={<div class="empty">{selected()[0] ? "Loading…" : "Select a commit"}</div>}>
          {(d) => (
            <div class="commit-detail">
              <div class="commit-meta">
                <h3>{d().commit.summary}</h3>
                <Show when={d().body.trim()}>
                  <pre>{d().body.trim()}</pre>
                </Show>
                <div class="muted" style={{ "font-size": "12px" }}>
                  {d().commit.authorName} &lt;{d().commit.authorEmail}&gt; · {absoluteTime(d().commit.time)} ·{" "}
                  <span style={{ "font-family": "var(--font-mono)", "user-select": "text" }}>{d().commit.shortId}</span> ·{" "}
                  {d().files.length} changed file{d().files.length === 1 ? "" : "s"}
                </div>
              </div>
              {body()}
            </div>
          )}
        </Show>
      }
    >
      {(r) => (
        <div class="commit-detail">
          <div class="commit-meta">
            <h3>Showing changes from {r().count} commits</h3>
            <div class="muted" style={{ "font-size": "12px" }}>
              {files().length} changed file{files().length === 1 ? "" : "s"} · select a single commit to see its details
            </div>
          </div>
          {body()}
        </div>
      )}
    </Show>
  );
}

export function focusCommit(sha: string) {
  setStore({ tab: "history", historySearch: null, compareWith: null });
  setSelected([sha]);
  setAnchor(sha);
  if (!commits().some((c) => c.id === sha)) {
    setStore("historySearch", { kind: "message", query: sha });
  }
}
