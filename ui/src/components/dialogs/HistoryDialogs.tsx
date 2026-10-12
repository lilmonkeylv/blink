import { ask } from "@tauri-apps/plugin-dialog";
import { createMemo, createResource, createSignal, For, Index, Show } from "solid-js";
import { createStore } from "solid-js/store";
import { api, type CommitSummary, type RebaseAction, type ResetMode } from "../../api";
import { absoluteTime, relativeTime } from "../../lib/time";
import { currentBranch, notify, run, store } from "../../store";
import { Field, Select } from "../Controls";
import { Dialog } from "../Dialog";
import { DiffView, Tokens } from "../DiffView";
import { focusCommit } from "../HistoryPane";
import { DownIcon, UpIcon } from "../Icons";
import { VirtualList } from "../VirtualList";
import { closeDialog, Tabs } from "./common";

const undoAction = (id: string) => ({ label: "Undo", run: () => run("Undo", () => api.undo(id).then(() => undefined)) });

export function TagDialog(p: { sha: string }) {
  const [name, setName] = createSignal("");
  const [message, setMessage] = createSignal("");
  const valid = () => /^[^\s~^:?*[\\]+$/.test(name().trim()) && !name().endsWith(".");
  const create = async () => {
    const repo = store.repo;
    if (!repo || !valid()) return;
    const ok = await run("Create tag", () => api.createTag(repo.id, name().trim(), p.sha, message() || undefined).then(() => true));
    if (ok) {
      closeDialog();
      notify("success", `Created tag ${name().trim()}. Push with tags to publish it.`, {
        action: { label: "Push tags", run: () => run("Push tags", () => api.sync(repo.id, "push", false, true)) },
      });
    }
  };
  return (
    <Dialog
      title={`Create a tag at ${p.sha.slice(0, 7)}`}
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!valid()} onClick={create}>
            Create tag
          </button>
        </>
      }
    >
      <Field label="Name">
        <input class="input" autofocus placeholder="v1.0.0" value={name()} onInput={(e) => setName(e.currentTarget.value)} />
      </Field>
      <Field label="Message" hint="Optional. A message creates an annotated tag.">
        <input class="input" value={message()} onInput={(e) => setMessage(e.currentTarget.value)} />
      </Field>
    </Dialog>
  );
}

const RESET_MODES: { value: ResetMode; title: string; detail: string }[] = [
  { value: "soft", title: "Soft", detail: "Move the branch; keep all changes staged." },
  { value: "mixed", title: "Mixed", detail: "Move the branch; keep changes in your working directory." },
  { value: "hard", title: "Hard", detail: "Move the branch and discard every change after this commit." },
];

export function ResetDialog(p: { sha: string }) {
  const [mode, setMode] = createSignal<ResetMode>("mixed");
  const reset = async () => {
    const repo = store.repo;
    if (!repo) return;
    if (mode() === "hard" && (repo.files.length || store.settings?.prompts.hardReset !== false)) {
      const ok = await ask("Hard reset discards uncommitted changes and moves the branch. Continue?", {
        title: "Hard reset",
        kind: "warning",
        okLabel: "Reset",
      });
      if (!ok) return;
    }
    closeDialog();
    const ok = await run("Reset", () => api.reset(repo.id, p.sha, mode()).then(() => true));
    if (ok) notify("success", `Reset ${currentBranch() ?? "HEAD"} to ${p.sha.slice(0, 7)}`, { action: undoAction(repo.id) });
  };
  return (
    <Dialog
      title={`Reset ${currentBranch() ?? "HEAD"} to ${p.sha.slice(0, 7)}`}
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" classList={{ danger: mode() === "hard" }} onClick={reset}>
            Reset
          </button>
        </>
      }
    >
      <div class="option-grid">
        <For each={RESET_MODES}>
          {(m) => (
            <button class="option-card" classList={{ active: mode() === m.value }} onClick={() => setMode(m.value)}>
              <strong>{m.title}</strong>
              <span class="muted">{m.detail}</span>
            </button>
          )}
        </For>
      </div>
    </Dialog>
  );
}

interface Step {
  commit: CommitSummary;
  action: RebaseAction;
  message: string;
}

const ACTIONS: { value: RebaseAction; label: string }[] = [
  { value: "pick", label: "Pick" },
  { value: "reword", label: "Reword" },
  { value: "squash", label: "Squash" },
  { value: "fixup", label: "Fixup" },
  { value: "drop", label: "Drop" },
];

export function InteractiveRebaseDialog(p: { sha: string }) {
  const [steps, setSteps] = createStore<Step[]>([]);
  const [base, setBase] = createSignal<string | null>(null);
  const [error, setError] = createSignal<string | null>(null);

  (async () => {
    const repo = store.repo!;
    const history = await api.history(repo.id, 0, 500);
    const end = history.findIndex((c) => c.id === p.sha);
    if (end < 0) return setError("That commit is not in the latest 500 commits of this branch.");
    const range = history.slice(0, end + 1);
    if (range.some((c) => c.parents.length > 1)) return setError("This range contains merge commits, which cannot be rewritten here.");
    setBase(range[range.length - 1].parents[0] ?? null);
    setSteps(range.reverse().map((commit) => ({ commit, action: "pick", message: commit.summary })));
  })().catch((e) => setError(String(e?.message ?? e)));

  const move = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= steps.length) return;
    const next = [...steps];
    [next[i], next[j]] = [next[j], next[i]];
    setSteps(next);
  };
  const invalid = createMemo(() => {
    const first = steps.find((s) => s.action !== "drop");
    if (!first) return "Keep at least one commit.";
    if (first.action === "squash" || first.action === "fixup") return "The first commit cannot be squashed into a previous one.";
    return null;
  });

  const apply = async () => {
    const repo = store.repo!;
    if (invalid()) return;
    closeDialog();
    const ok = await run("Rewrite history", () =>
      api
        .rebaseInteractive(repo.id, {
          base: base(),
          label: "Edit history",
          steps: steps.map((s) => ({
            action: s.action,
            sha: s.commit.id,
            message: s.action === "reword" || s.action === "squash" ? s.message : null,
          })),
        })
        .then(() => true),
    );
    if (ok) notify("success", "History rewritten", { action: undoAction(repo.id) });
  };

  return (
    <Dialog
      title="Edit history"
      onClose={closeDialog}
      wide
      footer={
        <>
          <span class="warning" style={{ "margin-right": "auto" }}>
            {invalid() ?? ""}
          </span>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!!error() || !!invalid() || !steps.length} onClick={apply}>
            Rewrite {steps.length} commit{steps.length === 1 ? "" : "s"}
          </button>
        </>
      }
    >
      <Show when={!error()} fallback={<div class="warning">{error()}</div>}>
        <span class="muted small">Oldest commit first. A backup is kept so you can undo with Ctrl+Z.</span>
        <div class="pick-list" style={{ "max-height": "380px" }}>
          <Index each={steps}>
            {(step, i) => (
              <div class="rebase-row" classList={{ dropped: step().action === "drop" }}>
                <Select value={step().action} onChange={(v) => setSteps(i, "action", v)} options={ACTIONS} ariaLabel="Action" />
                <Show
                  when={step().action === "reword" || step().action === "squash"}
                  fallback={
                    <span class="summary">
                      <span class="sha">{step().commit.shortId}</span> {step().commit.summary}
                    </span>
                  }
                >
                  <input class="input" value={step().message} onInput={(e) => setSteps(i, "message", e.currentTarget.value)} />
                </Show>
                <span class="summary-row">
                  <button class="icon-button" aria-label="Move up" disabled={i === 0} onClick={() => move(i, -1)}>
                    <UpIcon />
                  </button>
                  <button class="icon-button" aria-label="Move down" disabled={i === steps.length - 1} onClick={() => move(i, 1)}>
                    <DownIcon />
                  </button>
                </span>
              </div>
            )}
          </Index>
        </div>
      </Show>
    </Dialog>
  );
}

export function SquashDialog(p: { target: string; commits: string[] }) {
  const [message, setMessage] = createSignal("");
  (async () => {
    const repo = store.repo!;
    const all = [p.target, ...p.commits];
    const details = await Promise.all(all.map((sha) => api.commitDetail(repo.id, sha)));
    setMessage(details.map((d) => [d.commit.summary, d.body.trim()].filter(Boolean).join("\n\n")).join("\n\n"));
  })().catch(() => {});

  const squash = async () => {
    const repo = store.repo!;
    closeDialog();
    const ok = await run("Squash", () => api.squash(repo.id, p.target, p.commits, message()).then(() => true));
    if (ok) notify("success", `Squashed ${p.commits.length + 1} commits`, { action: undoAction(repo.id) });
  };

  return (
    <Dialog
      title={`Squash ${p.commits.length + 1} commits`}
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!message().trim()} onClick={squash}>
            Squash commits
          </button>
        </>
      }
    >
      <Field label="Commit message">
        <textarea class="textarea" style={{ "min-height": "180px" }} value={message()} onInput={(e) => setMessage(e.currentTarget.value)} />
      </Field>
    </Dialog>
  );
}

export function UndoHistoryDialog() {
  const [tab, setTab] = createSignal<"undo" | "reflog">("undo");
  const id = () => store.repo!.id;
  const [entries, { refetch }] = createResource(() => store.repo?.id, (i) => api.undoHistory(i));
  const [reflog] = createResource(() => (tab() === "reflog" ? store.repo?.id : null), (i) => api.reflog(i));
  const undo = async () => {
    await run("Undo", () => api.undo(id()).then(() => undefined));
    refetch();
  };
  return (
    <Dialog title="Undo history" onClose={closeDialog} wide>
      <Tabs
        value={tab()}
        onChange={setTab}
        tabs={[
          ["undo", "Blink operations"],
          ["reflog", "Reflog"],
        ]}
      />
      <Show when={tab() === "undo"}>
        <Show when={entries()?.length} fallback={<div class="empty muted">Nothing to undo in this session</div>}>
          <For each={entries()}>
            {(e, i) => (
              <div class="static-row">
                <span class="grow">{e.label}</span>
                <span class="muted small">{e.action.type === "moveBranch" ? `was ${e.action.sha.slice(0, 7)}` : `branch ${e.action.name}`}</span>
                <Show when={i() === 0}>
                  <button class="btn primary" onClick={undo}>
                    Undo
                  </button>
                </Show>
              </div>
            )}
          </For>
        </Show>
      </Show>
      <Show when={tab() === "reflog"}>
        <div class="pick-list" style={{ "max-height": "400px" }}>
          <For each={reflog()} fallback={<div class="empty muted">Loading…</div>}>
            {(r) => (
              <div class="static-row" title={r.sha}>
                <span class="muted small" style={{ width: "70px", "font-family": "var(--font-mono)" }}>
                  {r.sha.slice(0, 7)}
                </span>
                <span class="grow">{r.subject}</span>
                <span class="muted small">{relativeTime(r.time)}</span>
                <button
                  class="btn"
                  onClick={() => {
                    closeDialog();
                    focusCommit(r.sha);
                  }}
                >
                  Show
                </button>
              </div>
            )}
          </For>
        </div>
      </Show>
    </Dialog>
  );
}

export function FileHistoryDialog(p: { path: string }) {
  const [list] = createResource(() => store.repo?.id, (id) => api.fileHistory(id, p.path, 0, 500));
  const [picked, setPicked] = createSignal<string | null>(null);
  const current = () => picked() ?? list()?.[0]?.id ?? null;
  const [diff] = createResource(
    () => (store.repo && current() ? { id: store.repo.id, sha: current()! } : null),
    ({ id, sha }) => api.commitFileDiff(id, sha, p.path, null),
  );
  return (
    <Dialog title={`History of ${p.path}`} onClose={closeDialog} class="fill">
      <div class="commit-split" style={{ height: "100%", "grid-template-columns": "320px 1fr" }}>
        <VirtualList
          items={list() ?? []}
          rowHeight={52}
          render={(c) => (
            <div class="row history-row" classList={{ selected: c.id === current() }} onMouseDown={() => setPicked(c.id)} onDblClick={() => { closeDialog(); focusCommit(c.id); }}>
              <div class="history-text">
                <span class="summary">{c.summary}</span>
                <span class="meta">
                  {c.authorName} · {relativeTime(c.time)} · {c.shortId}
                </span>
              </div>
            </div>
          )}
        />
        <div class="content">
          <Show when={diff()} fallback={<div class="empty muted">{list.loading ? "Loading…" : "Select a commit"}</div>}>
            {(d) => <DiffView diff={d()} />}
          </Show>
        </div>
      </div>
    </Dialog>
  );
}

export function BlameDialog(p: { path: string }) {
  const [blame] = createResource(() => store.repo?.id, (id) => api.blame(id, p.path));
  const [hover, setHover] = createSignal<string | null>(null);
  const lines = () => blame()?.lines ?? [];
  return (
    <Dialog title={`Blame ${p.path}`} onClose={closeDialog} class="fill">
      <Show when={blame()} fallback={<div class="empty muted">Loading…</div>}>
        {(b) => (
          <VirtualList
            items={lines()}
            rowHeight={20}
            class="diff"
            render={(line, i) => {
              const commit = () => b().commits[line.sha];
              const first = () => i === 0 || lines()[i - 1].sha !== line.sha;
              return (
                <div class="blame-row" classList={{ first: first(), "diff-row": hover() === line.sha }} onMouseEnter={() => setHover(line.sha)}>
                  <span
                    class="who"
                    title={commit() ? `${commit()!.summary}\n${commit()!.author} · ${absoluteTime(commit()!.time)}` : line.sha}
                    onClick={() => {
                      closeDialog();
                      focusCommit(line.sha);
                    }}
                  >
                    {commit() ? `${relativeTime(commit()!.time)} · ${commit()!.author} · ${commit()!.summary}` : line.sha.slice(0, 7)}
                  </span>
                  <span class="no">{line.line}</span>
                  <span class="code">
                    <Tokens line={{ text: line.text }} />
                  </span>
                </div>
              );
            }}
          />
        )}
      </Show>
    </Dialog>
  );
}
