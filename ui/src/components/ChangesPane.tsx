import { ask } from "@tauri-apps/plugin-dialog";
import { createEffect, createMemo, createSignal, on, Show } from "solid-js";
import { api, type FileChange, type FileSelection } from "../api";
import { checkState, diffSignature, setLines, toggleFile, toPayload, type CheckState, type FileSel } from "../lib/selection";
import { cachedDiff, currentBranch, notify, refreshRepo, reportError, run, setStore, store, storeDiff } from "../store";
import { openContextMenu, type MenuItem } from "./ContextMenu";
import { Checkbox } from "./Controls";
import { DiffView } from "./DiffView";
import { StatusIcon } from "./Icons";
import { ImageDiff, isImagePath } from "./ImageDiff";
import { VirtualList } from "./VirtualList";

const SUMMARY_SOFT_LIMIT = 72;

const toCheck = (state: CheckState) => (state === "all" ? true : state === "partial" ? "mixed" : false);

export function fileMenu(f: { path: string; kind?: string }, extra: MenuItem[] = []): MenuItem[] {
  const repo = store.repo!;
  const tracked = f.kind !== "untracked" && f.kind !== "added";
  return [
    ...extra,
    { label: "Open in external editor", run: () => api.openInEditor(repo.id, f.path).catch(reportError) },
    { label: "Show in file manager", run: () => api.reveal(repo.id, f.path).catch(reportError) },
    { label: "Copy file path", run: () => navigator.clipboard.writeText(f.path) },
    "separator",
    { label: "File history", run: () => setStore("dialog", { type: "fileHistory", path: f.path }), disabled: !tracked },
    { label: "Blame", run: () => setStore("dialog", { type: "blame", path: f.path }), disabled: !tracked },
  ];
}

export function ChangesList() {
  const files = () => store.repo?.files ?? [];
  const selectedIndex = () => files().findIndex((f) => f.path === store.selectedPath);
  const fileState = (f: FileChange) => checkState(store.checkedPaths[f.path], cachedDiff(f.path));
  const allState = createMemo<CheckState>(() => {
    const states = files().map(fileState);
    if (states.every((s) => s === "all")) return "all";
    if (states.every((s) => s === "none")) return "none";
    return "partial";
  });

  const toggle = (f: FileChange) =>
    setStore("checkedPaths", f.path, toggleFile(store.checkedPaths[f.path], cachedDiff(f.path)));

  const toggleAll = () => {
    const include = allState() !== "all";
    const next: Record<string, FileSel> = {};
    for (const f of files()) next[f.path] = { included: include, excluded: new Set() };
    setStore("checkedPaths", next);
  };

  const discard = async (paths: string[]) => {
    const repo = store.repo;
    if (!repo || !paths.length) return;
    if (store.settings?.prompts.discardChanges !== false) {
      const label = paths.length === 1 ? paths[0] : `${paths.length} files`;
      const ok = await ask(`Discard changes to ${label}? Untracked files are moved to the trash.`, {
        title: "Discard changes",
        kind: "warning",
        okLabel: "Discard",
      });
      if (!ok) return;
    }
    await run("Discard changes", () => api.discardFiles(repo.id, paths));
  };

  const ignore = (pattern: string) => {
    const repo = store.repo;
    if (repo) run("Ignore", () => api.ignore(repo.id, pattern));
  };

  const menu = (e: MouseEvent, f: FileChange) => {
    const repo = store.repo!;
    const ext = f.path.includes(".") ? f.path.slice(f.path.lastIndexOf(".")) : null;
    openContextMenu(
      e,
      fileMenu(f, [
        { label: "Discard changes…", run: () => discard([f.path]) },
        { label: "Discard all changes…", run: () => discard(files().map((x) => x.path)) },
        { label: "Stash this file", run: () => run("Stash", () => api.stash(repo.id, `Stashed ${f.path}`, [f.path])) },
        "separator",
        { label: "Ignore file (add to .gitignore)", run: () => ignore(f.path), disabled: f.kind !== "untracked" },
        ...(ext ? [{ label: `Ignore all ${ext} files`, run: () => ignore(`*${ext}`) }] : []),
        "separator",
      ]),
    );
  };

  const onKeyDown = (e: KeyboardEvent) => {
    const list = files();
    const i = selectedIndex();
    if (e.key === "ArrowDown" || e.key === "j") {
      e.preventDefault();
      if (i < list.length - 1) setStore("selectedPath", list[i + 1].path);
    } else if (e.key === "ArrowUp" || e.key === "k") {
      e.preventDefault();
      if (i > 0) setStore("selectedPath", list[i - 1].path);
    } else if (e.key === " " && i >= 0) {
      e.preventDefault();
      toggle(list[i]);
    } else if (e.key === "Delete" && i >= 0) {
      discard([list[i].path]);
    }
  };

  return (
    <>
      <Show when={store.repo?.operation}>
        <OperationBar />
      </Show>
      <div class="list-header">
        <Checkbox checked={toCheck(allState())} onChange={toggleAll} ariaLabel="Include all changes" />
        <span>
          {files().length} changed file{files().length === 1 ? "" : "s"}
        </span>
      </div>
      <Show
        when={files().length}
        fallback={
          <div class="empty">
            <div>
              <h2>No local changes</h2>
              <p>There are no uncommitted changes in this repository.</p>
              <Show when={store.repo}>
                <div style={{ display: "flex", gap: "8px", "justify-content": "center", "flex-wrap": "wrap" }}>
                  <button class="btn" onClick={() => api.openInEditor(store.repo!.id).catch(reportError)}>
                    Open in editor
                  </button>
                  <button class="btn" onClick={() => api.reveal(store.repo!.id).catch(reportError)}>
                    Show in file manager
                  </button>
                </div>
              </Show>
            </div>
          </div>
        }
      >
        <VirtualList
          items={files()}
          rowHeight={store.settings?.appearance.density === "compact" ? 24 : 30}
          onKeyDown={onKeyDown}
          scrollToIndex={selectedIndex()}
          render={(f) => (
            <div
              class="row"
              classList={{ selected: f.path === store.selectedPath }}
              onMouseDown={() => setStore("selectedPath", f.path)}
              onContextMenu={(e) => menu(e, f)}
              title={f.oldPath ? `${f.oldPath} -> ${f.path}` : f.path}
            >
              <Checkbox
                checked={toCheck(fileState(f))}
                onChange={() => toggle(f)}
                ariaLabel={`Include ${f.path}`}
                stopPropagation
              />
              <span class="path">
                <bdi>{f.oldPath ? `${f.oldPath} -> ${f.path}` : f.path}</bdi>
              </span>
              <StatusIcon kind={f.kind} />
            </div>
          )}
        />
      </Show>
      <CommitBox />
    </>
  );
}

const OPERATION_LABELS: Record<string, string> = {
  merge: "Merge",
  rebase: "Rebase",
  "cherry-pick": "Cherry-pick",
  revert: "Revert",
};

function OperationBar() {
  const repo = () => store.repo!;
  const op = () => repo().operation!;
  const conflicts = () => repo().conflicted;
  return (
    <div class="operation-bar">
      <span style={{ flex: 1 }}>
        {OPERATION_LABELS[op()] ?? op()} in progress
        {conflicts() ? ` · ${conflicts()} conflicted file${conflicts() === 1 ? "" : "s"}` : " · conflicts resolved"}
      </span>
      <Show when={op() === "rebase" || op() === "cherry-pick"}>
        <button class="btn" onClick={() => run("Skip", () => api.skipOperation(repo().id, op()))}>
          Skip
        </button>
      </Show>
      <button class="btn" onClick={() => run("Abort", () => api.abortOperation(repo().id, op()))}>
        Abort
      </button>
      <button
        class="btn primary"
        disabled={conflicts() > 0}
        onClick={() => run("Continue", () => api.continueOperation(repo().id, op()))}
      >
        {op() === "merge" ? "Commit merge" : "Continue"}
      </button>
    </div>
  );
}

function CommitBox() {
  const [summary, setSummary] = createSignal("");
  const [description, setDescription] = createSignal("");
  const [coAuthors, setCoAuthors] = createSignal("");
  const [showCoAuthors, setShowCoAuthors] = createSignal(false);
  const [amend, setAmend] = createSignal(false);
  const [committing, setCommitting] = createSignal(false);

  const branch = () => currentBranch() ?? "detached HEAD";
  const anySelected = () =>
    (store.repo?.files ?? []).some((f) => checkState(store.checkedPaths[f.path], cachedDiff(f.path)) !== "none");
  const conflicted = () => (store.repo?.conflicted ?? 0) > 0;
  const midOperation = () => !!store.repo?.operation;
  const canCommit = () =>
    !committing() && summary().trim().length > 0 && (anySelected() || amend()) && !conflicted() && !midOperation();

  createEffect(
    on(amend, async (enabled) => {
      if (!enabled || summary() || !store.repo) return;
      const [last] = await api.history(store.repo.id, 0, 1).catch(() => []);
      if (!last) return;
      const detail = await api.commitDetail(store.repo.id, last.id).catch(() => null);
      setSummary(last.summary);
      setDescription(detail?.body.trim() ?? "");
    }),
  );

  const gatherSelection = async (id: string): Promise<FileSelection[] | null> => {
    const out: FileSelection[] = [];
    for (const f of store.repo?.files ?? []) {
      const sel = store.checkedPaths[f.path];
      if (sel && sel.included && sel.excluded.size > 0) {
        const diff = await api.fileDiff(id, f.path, false, true);
        if (sel.signature && sel.signature !== diffSignature(diff)) {
          notify("error", `${f.path} changed since lines were selected. Review the selection and try again.`);
          storeDiff(f.path, diff);
          return null;
        }
        const payload = toPayload(f.path, sel, diff);
        if (payload) out.push(payload);
      } else {
        const payload = toPayload(f.path, sel);
        if (payload) out.push(payload);
      }
    }
    return out;
  };

  const commit = async () => {
    const repo = store.repo;
    if (!repo || !canCommit()) return;
    setCommitting(true);
    try {
      const files = await gatherSelection(repo.id);
      if (!files) return;
      const authors = coAuthors()
        .split(/[\n,]/)
        .map((s) => s.trim())
        .filter(Boolean);
      await api.commit(repo.id, files, { summary: summary(), description: description(), coAuthors: authors }, amend());
      const count = files.length;
      const amended = amend();
      setSummary("");
      setDescription("");
      setAmend(false);
      setStore("checkedPaths", {});
      notify(
        "success",
        amended ? "Amended last commit" : `Committed ${count} file${count === 1 ? "" : "s"} to ${branch()}`,
        { action: { label: "Undo", run: () => run("Undo", () => api.undo(repo.id).then(() => undefined)) } },
      );
    } catch (e) {
      reportError(e, "Commit failed");
    } finally {
      setCommitting(false);
    }
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      commit();
    }
  };

  return (
    <div class="commit-box" onKeyDown={onKey}>
      <div class="summary-row">
        <input
          class="input"
          placeholder={amend() ? "Summary (amending last commit)" : "Summary (required)"}
          value={summary()}
          onInput={(e) => setSummary(e.currentTarget.value)}
        />
        <span class="char-count" classList={{ over: summary().length > SUMMARY_SOFT_LIMIT }}>
          {summary().length > 50 ? SUMMARY_SOFT_LIMIT - summary().length : ""}
        </span>
      </div>
      <textarea
        class="textarea"
        placeholder="Description"
        value={description()}
        onInput={(e) => setDescription(e.currentTarget.value)}
      />
      <Show when={showCoAuthors()}>
        <textarea
          class="textarea"
          style={{ "min-height": "44px" }}
          placeholder="Co-authors: Name <email>, one per line"
          value={coAuthors()}
          onInput={(e) => setCoAuthors(e.currentTarget.value)}
        />
      </Show>
      <div class="summary-row" style={{ "justify-content": "space-between" }}>
        <Checkbox checked={amend()} onChange={setAmend} label="Amend last commit" class="muted small" />
        <a style={{ "font-size": "12px" }} onClick={() => setShowCoAuthors(!showCoAuthors())}>
          {showCoAuthors() ? "Remove co-authors" : "Add co-authors"}
        </a>
      </div>
      <Show when={summary().length > SUMMARY_SOFT_LIMIT}>
        <div class="warning">Great commit summaries are 50 characters or fewer.</div>
      </Show>
      <Show when={conflicted()}>
        <div class="warning">Resolve all conflicts before committing.</div>
      </Show>
      <button class="btn primary block" disabled={!canCommit()} onClick={commit} title="Ctrl+Enter">
        {committing() ? "Committing…" : amend() ? "Amend last commit" : `Commit to ${branch()}`}
      </button>
    </div>
  );
}

export function ChangesDiff() {
  const [ignoreWhitespace, setIgnoreWhitespace] = createSignal(false);
  const [full, setFull] = createSignal(false);
  const [wsDiff, setWsDiff] = createSignal<ReturnType<typeof cachedDiff>>();
  const file = () => store.repo?.files.find((f) => f.path === store.selectedPath) ?? null;
  let token = 0;

  createEffect(
    on(
      () => [store.repo?.id, store.repo?.generation, store.selectedPath, ignoreWhitespace(), full()] as const,
      async ([id, , path, ws, loadFull]) => {
        if (!id || !path || isImagePath(path)) return;
        if (!ws && !loadFull && cachedDiff(path)) return;
        const mine = ++token;
        try {
          const diff = await api.fileDiff(id, path, ws, loadFull);
          if (mine !== token) return;
          if (ws) setWsDiff(diff);
          else storeDiff(path, diff);
        } catch (e) {
          if (mine === token) reportError(e);
        }
      },
    ),
  );
  createEffect(on(() => store.selectedPath, () => setFull(false)));

  const diff = () => (ignoreWhitespace() ? wsDiff() : file() ? cachedDiff(file()!.path) : undefined);

  const setLinesFor = (keys: string[], include: boolean) => {
    const f = file();
    const d = f && cachedDiff(f.path);
    if (!f || !d || ignoreWhitespace()) return;
    setStore("checkedPaths", f.path, setLines(store.checkedPaths[f.path], keys, include, d));
  };

  const discardLines = async (keys: string[]) => {
    const f = file();
    const repo = store.repo;
    if (!f || !repo || !keys.length) return;
    const ok = await ask(`Discard ${keys.length} selected line${keys.length === 1 ? "" : "s"} in ${f.path}?`, {
      title: "Discard lines",
      kind: "warning",
      okLabel: "Discard",
    });
    if (!ok) return;
    const lines = keys.map((k) => k.split(":").map(Number) as [number, number]);
    await run("Discard lines", () => api.discardLines(repo.id, f.path, lines));
    await refreshRepo();
  };

  return (
    <Show when={file()} fallback={<div class="empty">Select a file to view its changes</div>}>
      {(f) => (
        <>
          <div class="diff-header">
            <StatusIcon kind={f().kind} />
            <span class="path">{f().oldPath ? `${f().oldPath} -> ${f().path}` : f().path}</span>
            <Show when={diff()}>
              {(d) => (
                <span class="muted">
                  <span style={{ color: "var(--add-fg)" }}>+{d().additions}</span>{" "}
                  <span style={{ color: "var(--del-fg)" }}>-{d().deletions}</span>
                </span>
              )}
            </Show>
            <Checkbox checked={ignoreWhitespace()} onChange={setIgnoreWhitespace} label="Hide whitespace" class="muted small" />
          </div>
          <Show when={f().kind === "conflicted"}>
            <ConflictActions path={f().path} />
          </Show>
          <Show when={!isImagePath(f().path)} fallback={<ImageDiff path={f().path} oldPath={f().oldPath} />}>
            <Show when={diff()} fallback={<div class="empty muted">Loading…</div>}>
              {(d) => (
                <DiffView
                  diff={d()}
                  selectable={!ignoreWhitespace() && f().kind !== "conflicted"}
                  sel={store.checkedPaths[f().path]}
                  onSetLines={setLinesFor}
                  onDiscardLines={f().kind === "untracked" ? undefined : discardLines}
                  onLoadAnyway={() => setFull(true)}
                />
              )}
            </Show>
          </Show>
        </>
      )}
    </Show>
  );
}

function ConflictActions(p: { path: string }) {
  const repo = () => store.repo!;
  return (
    <div class="operation-bar" style={{ background: "var(--bg-subtle)" }}>
      <span style={{ flex: 1 }}>This file has conflicts.</span>
      <button class="btn primary" onClick={() => setStore("dialog", { type: "mergeEditor", path: p.path })}>
        Resolve…
      </button>
      <button class="btn" onClick={() => api.openInEditor(repo().id, p.path).catch(reportError)}>
        Open in editor
      </button>
      <button class="btn" onClick={() => run("Resolve", () => api.resolveConflict(repo().id, p.path, true))}>
        Use ours
      </button>
      <button class="btn" onClick={() => run("Resolve", () => api.resolveConflict(repo().id, p.path, false))}>
        Use theirs
      </button>
    </div>
  );
}
