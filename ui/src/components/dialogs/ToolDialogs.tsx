import { getVersion } from "@tauri-apps/api/app";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { createMemo, createResource, createSignal, For, Match, onCleanup, Show, Switch } from "solid-js";
import { api, isCommandError, type CheckRun, type DeviceCode, type JobStep, type ReleaseNote } from "../../api";
import { baseSections, conflictCount, parseConflicts, render, type Resolution, type Segment } from "../../lib/conflicts";
import { parseJobLog } from "../../lib/joblog";
import { COMMIT_KEYS, DEFAULT_BINDINGS } from "../../lib/keymap";
import logo from "../../../../assets/logo.svg";
import { addRepository, checkForUpdates, keymap, notify, refreshChecks, reportError, run, setStore, store } from "../../store";
import { Checkbox, Field, Segmented } from "../Controls";
import { Dialog } from "../Dialog";
import { CheckIcon, ChevronRight, ExternalIcon, FolderOpenIcon } from "../Icons";
import { Loading } from "../Loading";
import { Markdown } from "../Markdown";
import { VirtualList } from "../VirtualList";
import { closeDialog, sanitizeBranchName } from "./common";

type SignedIn = Awaited<ReturnType<typeof api.signIn>>;

export function SignInDialog() {
  const [mode, setMode] = createSignal<"idle" | "browser" | "device">("idle");
  const [device, setDevice] = createSignal<DeviceCode | null>(null);
  const [copied, setCopied] = createSignal(false);
  let attempt = 0;
  let copiedTimer: number | undefined;
  onCleanup(() => {
    attempt++;
    clearTimeout(copiedTimer);
  });

  const finish = async (signedIn: SignedIn) => {
    const boot = await api.bootstrap();
    setStore("account", boot.account);
    const identity = await api.gitIdentity().catch(() => null);
    if (identity && (!identity.name || !identity.email)) {
      await api.setGitIdentity(identity.name || signedIn.account.login, identity.email || signedIn.primaryEmail || "").catch(reportError);
    }
    notify("success", `Signed in as ${signedIn.account.login}`);
    refreshChecks();
    closeDialog();
  };

  const attemptSignIn = async (start: () => Promise<SignedIn>) => {
    const mine = ++attempt;
    try {
      const signedIn = await start();
      if (mine === attempt) await finish(signedIn);
    } catch (e) {
      if (mine !== attempt) return;
      reportError(e, "Sign in");
      setMode("idle");
    }
  };

  const browser = () => {
    setMode("browser");
    attemptSignIn(() => api.signIn());
  };

  const withCode = () => {
    setMode("device");
    setDevice(null);
    attemptSignIn(async () => {
      const code = await api.deviceStart();
      setDevice(code);
      return api.deviceComplete(code);
    });
  };

  const copyCode = async (code: string) => {
    await navigator.clipboard.writeText(code).catch(reportError);
    setCopied(true);
    clearTimeout(copiedTimer);
    copiedTimer = window.setTimeout(() => setCopied(false), 1500);
  };

  return (
    <Dialog title="Sign in to GitHub.com" onClose={closeDialog}>
      <Switch>
        <Match when={!store.oauthConfigured}>
          <p style={{ margin: 0 }}>
            GitHub sign-in isn't available in this build. You can sign in with a personal access token in Settings &gt; Accounts.
          </p>
          <button class="btn primary" onClick={() => setStore("dialog", { type: "settings", tab: "Accounts" })}>
            Open account settings
          </button>
        </Match>
        <Match when={mode() === "idle"}>
          <p style={{ margin: 0 }}>Your browser opens so you can sign in with your password, passkey or SSO. Blink never sees your password.</p>
          <button class="btn primary" onClick={browser}>
            Continue with browser
          </button>
          <button class="btn" onClick={withCode}>
            Sign in with a code
          </button>
        </Match>
        <Match when={mode() === "browser"}>
          <div class="sign-in-wait">
            <span class="spinner" />
            <span>Finish signing in in your browser. This window updates automatically.</span>
          </div>
          <button class="btn" onClick={withCode}>
            Use a code instead
          </button>
        </Match>
        <Match when={mode() === "device"}>
          <Show
            when={device()}
            fallback={
              <div class="sign-in-wait">
                <span class="spinner" />
                <span>Requesting a code…</span>
              </div>
            }
          >
            {(d) => (
              <>
                <p style={{ margin: 0 }}>
                  Enter this code at{" "}
                  <button type="button" class="link-button" onClick={() => api.openExternal(d().verification_uri).catch(reportError)}>
                    {d().verification_uri}
                  </button>
                </p>
                <div class="device-code">{d().user_code}</div>
                <button class="btn" onClick={() => copyCode(d().user_code)}>
                  <Show when={copied()} fallback="Copy code">
                    <CheckIcon />
                    Copied
                  </Show>
                </button>
                <div class="sign-in-wait muted">
                  <span class="spinner" />
                  <span>Waiting for you to enter the code on GitHub…</span>
                </div>
              </>
            )}
          </Show>
        </Match>
      </Switch>
    </Dialog>
  );
}

const FIXED_SHORTCUTS: [string, string][] = [
  [COMMIT_KEYS, "Commit"],
  ["J / K, Space", "Move and toggle lines in a diff"],
];

export function ShortcutsDialog() {
  return (
    <Dialog
      title="Keyboard shortcuts"
      onClose={closeDialog}
      footer={
        <>
          <span class="muted grow">Change any of these in keymap.toml.</span>
          <button class="btn" onClick={() => api.openKeymap().catch(reportError)}>
            Edit keymap.toml
          </button>
        </>
      }
    >
      <For each={DEFAULT_BINDINGS.filter((b) => keymap().keys[b.id])}>
        {(b) => (
          <div class="summary-row" style={{ "justify-content": "space-between" }}>
            <span>{b.label}</span>
            <span class="kbd">{keymap().keys[b.id]}</span>
          </div>
        )}
      </For>
      <For each={FIXED_SHORTCUTS}>
        {([keys, what]) => (
          <div class="summary-row" style={{ "justify-content": "space-between" }}>
            <span>{what}</span>
            <span class="kbd">{keys}</span>
          </div>
        )}
      </For>
    </Dialog>
  );
}

export function WorktreesDialog() {
  const [list, { refetch }] = createResource(() => store.repo?.id, (id) => api.worktrees(id));
  const [branch, setBranch] = createSignal("");
  const [path, setPath] = createSignal("");
  const [create, setCreate] = createSignal(true);
  const id = () => store.repo!.id;
  const choose = async () => {
    const dir = await openDialog({ directory: true, title: "Worktree location" });
    if (typeof dir === "string") setPath(`${dir}${dir.includes("\\") ? "\\" : "/"}${sanitizeBranchName(branch()) || "worktree"}`);
  };
  const add = async () => {
    const ok = await run("Add worktree", () => api.addWorktree(id(), path(), sanitizeBranchName(branch()), create()).then(() => true));
    if (!ok) return;
    refetch();
    notify("success", "Worktree created", { action: { label: "Open", run: () => addRepository(path()) } });
  };
  return (
    <Dialog title="Worktrees" onClose={closeDialog} wide>
      <For each={list()} fallback={<Loading />}>
        {(w, i) => (
          <div class="static-row">
            <span class="grow" title={w.path}>
              {w.path}
            </span>
            <span class="muted small">{w.branch ?? (w.head ? w.head.slice(0, 7) : "")}</span>
            <Show when={i() > 0}>
              <button class="btn" onClick={() => addRepository(w.path)}>
                Open
              </button>
              <button
                class="btn danger"
                onClick={async () => {
                  await run("Remove worktree", () => api.removeWorktree(id(), w.path, false));
                  refetch();
                }}
              >
                Remove
              </button>
            </Show>
          </div>
        )}
      </For>
      <h4 style={{ margin: "8px 0 0" }}>New worktree</h4>
      <Field label="Branch">
        <input class="input" value={branch()} onInput={(e) => setBranch(e.currentTarget.value)} />
      </Field>
      <Checkbox checked={create()} onChange={setCreate} label="Create a new branch" />
      <Field label="Location">
        <div class="summary-row">
          <input class="input" value={path()} onInput={(e) => setPath(e.currentTarget.value)} />
          <button class="btn" onClick={choose}>
            Choose…
          </button>
        </div>
      </Field>
      <div>
        <button class="btn primary" disabled={!branch().trim() || !path().trim()} onClick={add}>
          Add worktree
        </button>
      </div>
    </Dialog>
  );
}

export function ApplyPatchDialog() {
  const [patch, setPatch] = createSignal("");
  const apply = async () => {
    const ok = await run("Apply patch", () => api.applyPatch(store.repo!.id, patch()).then(() => true));
    if (ok) {
      closeDialog();
      notify("success", "Patch applied to your working directory");
    }
  };
  const paste = async () => setPatch(await navigator.clipboard.readText().catch(() => patch()));
  return (
    <Dialog
      title="Apply patch"
      onClose={closeDialog}
      wide
      footer={
        <>
          <button class="btn" style={{ "margin-right": "auto" }} onClick={paste}>
            Paste from clipboard
          </button>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={!patch().trim()} onClick={apply}>
            Apply
          </button>
        </>
      }
    >
      <textarea class="textarea code-area" spellcheck={false} placeholder="Paste a patch or diff" value={patch()} onInput={(e) => setPatch(e.currentTarget.value)} />
    </Dialog>
  );
}

const CONCLUSION_STATE = (status: string, conclusion: string | null) =>
  status !== "completed" ? "pending" : conclusion === "success" || conclusion === "skipped" || conclusion === "neutral" ? "success" : "failure";

export function ChecksDialog() {
  const checks = () => store.checks;
  const failedSuites = createMemo(() => [
    ...new Set(
      (checks()?.runs ?? [])
        .filter((r) => CONCLUSION_STATE(r.status, r.conclusion) === "failure" && r.check_suite)
        .map((r) => r.check_suite!.id),
    ),
  ]);
  const rerun = async () => {
    await run("Re-run checks", () => api.rerunFailedChecks(store.repo!.id, failedSuites()));
    notify("info", "Re-running failed checks");
    refreshChecks();
  };
  return (
    <Dialog
      title="Checks"
      onClose={closeDialog}
      wide
      footer={
        <>
          <button class="btn" onClick={refreshChecks}>
            Refresh
          </button>
          <button class="btn primary" disabled={!failedSuites().length} onClick={rerun}>
            Re-run failed checks
          </button>
        </>
      }
    >
      <Show when={checks()} fallback={<div class="empty muted">No checks for this commit</div>}>
        {(c) => (
          <>
            <For each={c().runs}>{(r) => <CheckRunRow run={r} />}</For>
            <For each={c().statuses}>
              {(s) => (
                <div class="check-run">
                  <span class={`dot ${s.state === "success" ? "success" : s.state === "pending" ? "pending" : "failure"}`} />
                  <span class="grow">{s.context}</span>
                  <span class="muted small">{s.state}</span>
                </div>
              )}
            </For>
          </>
        )}
      </Show>
    </Dialog>
  );
}

const viewLog = (jobId: number, name: string) => setStore("dialog", { type: "checkLog", jobId, name });

const jobState = (status: string, conclusion: string | null) =>
  conclusion === "skipped" || conclusion === "cancelled" ? "" : CONCLUSION_STATE(status, conclusion);

async function workflowRunId(repo: string, run: CheckRun) {
  const fromUrl = run.html_url?.match(/\/actions\/runs\/(\d+)/)?.[1];
  return fromUrl ? Number(fromUrl) : (await api.workflowJob(repo, run.id)).run_id;
}

function CheckRunRow(p: { run: CheckRun }) {
  const [open, setOpen] = createSignal(false);
  const actions = () => p.run.app?.slug === "github-actions";
  const state = () => CONCLUSION_STATE(p.run.status, p.run.conclusion);
  const [jobs] = createResource(
    () => (open() && actions() && store.repo ? store.repo.id : null),
    async (repo) => api.workflowRunJobs(repo, await workflowRunId(repo, p.run)),
  );
  return (
    <div class="check-group" classList={{ open: open() }}>
      <div class="check-run">
        <Show when={actions()} fallback={<span class="check-toggle-spacer" />}>
          <button
            type="button"
            class="icon-button check-toggle"
            aria-expanded={open()}
            aria-label={open() ? "Hide jobs" : "Show jobs"}
            onClick={() => setOpen(!open())}
          >
            <ChevronRight />
          </button>
        </Show>
        <span class={`dot ${state()}`} />
        <span class="grow">{p.run.name}</span>
        <span class="muted small">{p.run.conclusion ?? p.run.status.replace("_", " ")}</span>
        <Show when={actions() && state() === "failure"}>
          <button class="btn" onClick={() => viewLog(p.run.id, p.run.name)}>
            View log
          </button>
        </Show>
      </div>
      <Show when={open()}>
        <div class="check-jobs">
          <Switch fallback={<Loading />}>
            <Match when={jobs.error}>
              <div class="muted small">Could not load the jobs for this workflow run.</div>
            </Match>
            <Match when={jobs()}>
              {(list) => (
                <For each={list()} fallback={<div class="muted small">This workflow run has no jobs.</div>}>
                  {(job) => (
                    <div class="check-job" classList={{ current: job.id === p.run.id }}>
                      <span class={`dot ${jobState(job.status, job.conclusion)}`} />
                      <span class="grow">{job.name}</span>
                      <span class="muted small">{job.conclusion ?? job.status.replace("_", " ")}</span>
                      <button class="btn" disabled={job.status !== "completed"} onClick={() => viewLog(job.id, job.name)}>
                        View log
                      </button>
                    </div>
                  )}
                </For>
              )}
            </Match>
          </Switch>
        </div>
      </Show>
    </div>
  );
}

export function CheckLogDialog(p: { jobId: number; name: string }) {
  const id = store.repo!.id;
  const [job] = createResource(() => api.workflowJob(id, p.jobId));
  const [log] = createResource(async () => parseJobLog(await api.jobLog(id, p.jobId)));
  const steps = () => (job.error ? [] : (job()?.steps ?? []));
  const jobUrl = () => (job.error ? null : (job()?.html_url ?? null));
  const stepState = (s: JobStep) => (s.conclusion === "skipped" ? "" : CONCLUSION_STATE(s.status, s.conclusion));
  return (
    <Dialog
      title={`Log for ${p.name}`}
      onClose={closeDialog}
      class="fill"
      footer={
        <>
          <Show when={!log.error && log()?.hidden}>
            {(hidden) => (
              <span class="muted small" style={{ "margin-right": "auto" }}>
                {hidden()} earlier lines not shown
              </span>
            )}
          </Show>
          <Show when={jobUrl()}>
            {(url) => (
              <button class="btn" onClick={() => api.openExternal(url()).catch(reportError)}>
                Open on GitHub
              </button>
            )}
          </Show>
          <button class="btn primary" onClick={() => setStore("dialog", { type: "checks" })}>
            Back to checks
          </button>
        </>
      }
    >
      <Show when={steps().length}>
        <div class="log-steps">
          <For each={steps()}>
            {(s) => (
              <span class="log-step" classList={{ failed: stepState(s) === "failure" }}>
                <span class={`dot ${stepState(s)}`} />
                {s.name}
              </span>
            )}
          </For>
        </div>
      </Show>
      <Switch fallback={<Loading />}>
        <Match when={log.error}>
          <div class="empty muted">{String((log.error as { message?: string })?.message ?? "Could not load the log")}</div>
        </Match>
        <Match when={log()}>
          {(l) => (
            <VirtualList
              items={l().lines}
              rowHeight={20}
              class="diff job-log"
              scrollToIndex={l().firstError}
              render={(line) => <div class={`log-line ${line.kind}`}>{line.text}</div>}
            />
          )}
        </Match>
      </Switch>
    </Dialog>
  );
}

export function MergeEditorDialog(p: { path: string }) {
  const [segments, setSegments] = createSignal<Segment[]>([]);
  const [resolutions, setResolutions] = createSignal<Resolution[]>([]);
  const [manual, setManual] = createSignal<string | null>(null);
  const [view, setView] = createSignal<"guided" | "result">("guided");
  const [status, setStatus] = createSignal<"loading" | "ready" | "error">("loading");
  const [loadError, setLoadError] = createSignal("");
  const repoId = store.repo!.id;
  const [ancestor] = createResource(() => api.conflictVersions(repoId, p.path).then(([base]) => base, () => null));

  const load = async () => {
    setStatus("loading");
    try {
      const parsed = parseConflicts(await api.readWorktreeFile(repoId, p.path));
      setManual(null);
      setSegments(parsed);
      setResolutions(Array(conflictCount(parsed)).fill(null));
      setStatus("ready");
    } catch (e) {
      setLoadError(isCommandError(e) ? e.message : String(e));
      setStatus("error");
    }
  };
  void load();

  const output = createMemo(() => render(segments(), resolutions()));
  const conflicts = createMemo(() => segments().filter((s) => s.type === "conflict"));
  const bases = createMemo(() => {
    const text = ancestor();
    return text == null ? conflicts().map((c) => (c.type === "conflict" ? c.base : null)) : baseSections(segments(), text);
  });
  const baseNote = (i: number) => {
    if (bases()[i]) return null;
    if (ancestor.loading) return "Loading the common ancestor…";
    if (ancestor() == null) return "There is no common ancestor. Both sides added this file.";
    return "Could not find this part of the file in the common ancestor.";
  };
  const result = () => manual() ?? output().text;
  const unresolved = () => (manual() != null ? (manual()!.includes("<<<<<<<") ? 1 : 0) : output().unresolved);

  const choose = (i: number, r: Resolution) => {
    setManual(null);
    setResolutions((prev) => prev.map((x, j) => (j === i ? r : x)));
  };

  const save = async () => {
    if (status() !== "ready" || unresolved() > 0) return;
    const ok = await run("Resolve conflict", () => api.writeResolvedFile(repoId, p.path, result()).then(() => true));
    if (ok) {
      closeDialog();
      notify("success", `Resolved ${p.path}`);
    }
  };

  return (
    <Dialog
      title={`Resolve ${p.path}`}
      onClose={closeDialog}
      class="fill"
      footer={
        <>
          <span class="muted small" style={{ "margin-right": "auto" }}>
            <Show when={status() === "ready"}>
              {unresolved() ? `${unresolved()} conflict${unresolved() === 1 ? "" : "s"} left` : "All conflicts resolved"}
            </Show>
          </span>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={status() !== "ready" || unresolved() > 0} onClick={save}>
            Mark as resolved
          </button>
        </>
      }
    >
      <Switch>
        <Match when={status() === "loading"}>
          <div class="empty">
            <div class="loading">
              <span class="spinner large" />
              <span class="muted">Reading {p.path}…</span>
            </div>
          </div>
        </Match>
        <Match when={status() === "error"}>
          <div class="empty">
            <div>
              <p>Could not read {p.path}.</p>
              <p class="muted small">{loadError()}</p>
              <button class="btn" onClick={load}>
                Retry
              </button>
            </div>
          </div>
        </Match>
        <Match when={status() === "ready"}>
          <div class="history-tools" style={{ "justify-content": "space-between" }}>
            <Segmented
              value={view()}
              onChange={setView}
              options={[
                { value: "guided", label: `Conflicts (${conflicts().length})` },
                { value: "result", label: "Result" },
              ]}
            />
          </div>
          <div style={{ padding: "16px", overflow: "auto", display: "flex", "flex-direction": "column", gap: "12px", flex: 1 }}>
            <Show
              when={view() === "guided"}
              fallback={<textarea class="textarea code-area" style={{ flex: 1 }} spellcheck={false} value={result()} onInput={(e) => setManual(e.currentTarget.value)} />}
            >
              <For each={conflicts()} fallback={<div class="empty muted">No conflict markers found in this file.</div>}>
                {(c, i) =>
                  c.type === "conflict" && (
                    <div class="conflict-card">
                      <header>
                        <span class="grow">Conflict {i() + 1}</span>
                        <Segmented
                          value={resolutions()[i()] ?? ""}
                          onChange={(v) => choose(i(), (v || null) as Resolution)}
                          options={[
                            { value: "ours", label: "Ours" },
                            { value: "theirs", label: "Theirs" },
                            { value: "both", label: "Both" },
                            { value: "theirs-first", label: "Theirs, then ours" },
                          ]}
                        />
                      </header>
                      <div class="muted small" style={{ padding: "4px 10px" }}>
                        Ours ({c.oursLabel || "current"})
                      </div>
                      <pre class="conflict-side ours">{c.ours.join("\n") || " "}</pre>
                      <div class="muted small" style={{ padding: "4px 10px" }}>
                        Theirs ({c.theirsLabel || "incoming"})
                      </div>
                      <pre class="conflict-side theirs">{c.theirs.join("\n") || " "}</pre>
                      <BaseSection lines={bases()[i()]} note={baseNote(i())} />
                    </div>
                  )
                }
              </For>
            </Show>
          </div>
        </Match>
      </Switch>
    </Dialog>
  );
}

function BaseSection(p: { lines: string[] | null; note: string | null }) {
  const [open, setOpen] = createSignal(false);
  return (
    <div class="conflict-base" classList={{ open: open() }}>
      <button type="button" class="conflict-base-toggle" aria-expanded={open()} onClick={() => setOpen(!open())}>
        <ChevronRight />
        Base (common ancestor)
      </button>
      <Show when={open()}>
        <Show when={p.lines} fallback={<div class="muted small conflict-base-note">{p.note}</div>}>
          {(lines) => (
            <Show when={lines().length} fallback={<div class="muted small conflict-base-note">Empty in the common ancestor.</div>}>
              <pre class="conflict-side base">{lines().join("\n")}</pre>
            </Show>
          )}
        </Show>
      </Show>
    </div>
  );
}

export function WhatsNewDialog(p: { notes: ReleaseNote[] }) {
  return (
    <Dialog
      title="What's new in Blink"
      onClose={closeDialog}
      wide
      footer={
        <button class="btn primary" onClick={closeDialog}>
          Got it
        </button>
      }
    >
      <For each={p.notes}>
        {(note) => (
          <section class="release-note">
            <h3>
              {note.version}
              <Show when={note.date}>
                <span class="muted small"> · {note.date}</span>
              </Show>
            </h3>
            <Markdown source={note.body} />
          </section>
        )}
      </For>
    </Dialog>
  );
}

const REPO_URL = "https://github.com/lilmonkeylv/blink";

export function AboutDialog() {
  const [version] = createResource(() => getVersion().catch(() => null));
  const channel = () => (store.settings?.advanced.updateChannel === "beta" ? "Beta (prereleases)" : "Stable");
  const open = (url: string) => api.openExternal(url).catch(reportError);
  return (
    <Dialog
      title="About Blink"
      onClose={closeDialog}
      footer={
        <>
          <button class="btn" onClick={() => checkForUpdates(true)}>
            Check for updates
          </button>
          <button class="btn primary" onClick={closeDialog}>
            Close
          </button>
        </>
      }
    >
      <div class="about-head">
        <img src={logo} alt="" width="64" height="64" />
        <div>
          <h2>Blink</h2>
          <div class="muted">
            <Show when={!version.loading} fallback="Version…">
              Version <span class="about-version">{version() ?? "unknown"}</span>
            </Show>
          </div>
          <div class="muted small">Update channel: {channel()}</div>
        </div>
      </div>
      <div class="about-links">
        <button type="button" class="btn" onClick={() => open(REPO_URL)}>
          <ExternalIcon />
          GitHub repository
        </button>
        <button type="button" class="btn" onClick={() => open(`${REPO_URL}/releases`)}>
          <ExternalIcon />
          Releases
        </button>
        <button type="button" class="btn" onClick={() => api.openLogsFolder().catch(reportError)}>
          <FolderOpenIcon />
          Logs folder
        </button>
      </div>
    </Dialog>
  );
}
