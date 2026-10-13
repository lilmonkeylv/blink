import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { createMemo, createResource, createSignal, For, Match, Show, Switch } from "solid-js";
import { api, type DeviceCode, type JobStep, type ReleaseNote } from "../../api";
import { conflictCount, parseConflicts, render, type Resolution, type Segment } from "../../lib/conflicts";
import { parseJobLog } from "../../lib/joblog";
import { DEFAULT_BINDINGS } from "../../lib/keymap";
import { addRepository, keymap, notify, refreshChecks, reportError, run, setStore, store } from "../../store";
import { Checkbox, Field, Segmented } from "../Controls";
import { Dialog } from "../Dialog";
import { Markdown } from "../Markdown";
import { VirtualList } from "../VirtualList";
import { closeDialog, sanitizeBranchName } from "./common";

export function SignInDialog() {
  const [mode, setMode] = createSignal<"idle" | "browser" | "device">("idle");
  const [device, setDevice] = createSignal<DeviceCode | null>(null);

  const finish = async (signedIn: { account: { login: string }; primaryEmail: string | null } | undefined) => {
    if (!signedIn) return setMode("idle");
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

  const browser = async () => {
    setMode("browser");
    await finish(await run("Sign in", () => api.signIn()));
  };

  const withCode = async () => {
    setMode("device");
    const code = await run("Sign in", () => api.deviceStart());
    if (!code) return setMode("idle");
    setDevice(code);
    await finish(await run("Sign in", () => api.deviceComplete(code)));
  };

  return (
    <Dialog title="Sign in to GitHub.com" onClose={closeDialog}>
      <Switch>
        <Match when={!store.oauthConfigured}>
          <p style={{ margin: 0 }}>
            GitHub sign-in is not configured for this build. Set <code>BLINK_GITHUB_CLIENT_ID</code> (see <code>.env.example</code>) and
            restart.
          </p>
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
          <p style={{ margin: 0 }}>Finish signing in in your browser. This window updates automatically.</p>
        </Match>
        <Match when={mode() === "device"}>
          <Show when={device()} fallback={<p>Requesting a code…</p>}>
            {(d) => (
              <>
                <p style={{ margin: 0 }}>
                  Enter this code at <a>{d().verification_uri}</a>:
                </p>
                <div class="device-code">{d().user_code}</div>
                <button class="btn" onClick={() => navigator.clipboard.writeText(d().user_code)}>
                  Copy code
                </button>
              </>
            )}
          </Show>
        </Match>
      </Switch>
    </Dialog>
  );
}

const FIXED_SHORTCUTS: [string, string][] = [
  ["Ctrl+Enter", "Commit"],
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
      <For each={list()} fallback={<div class="empty muted">Loading…</div>}>
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
            <For each={c().runs}>
              {(r) => (
                <div class="check-run">
                  <span class={`dot ${CONCLUSION_STATE(r.status, r.conclusion)}`} />
                  <span class="grow">{r.name}</span>
                  <span class="muted small">{r.conclusion ?? r.status.replace("_", " ")}</span>
                  <Show when={r.app?.slug === "github-actions" && CONCLUSION_STATE(r.status, r.conclusion) === "failure"}>
                    <button class="btn" onClick={() => setStore("dialog", { type: "checkLog", jobId: r.id, name: r.name })}>
                      View log
                    </button>
                  </Show>
                </div>
              )}
            </For>
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
      <Switch fallback={<div class="empty muted">Loading…</div>}>
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

  api
    .readWorktreeFile(store.repo!.id, p.path)
    .then((text) => {
      const parsed = parseConflicts(text);
      setSegments(parsed);
      setResolutions(Array(conflictCount(parsed)).fill(null));
    })
    .catch(reportError);

  const output = createMemo(() => render(segments(), resolutions()));
  const conflicts = createMemo(() => segments().filter((s) => s.type === "conflict"));
  const result = () => manual() ?? output().text;
  const unresolved = () => (manual() != null ? (manual()!.includes("<<<<<<<") ? 1 : 0) : output().unresolved);

  const choose = (i: number, r: Resolution) => {
    setManual(null);
    setResolutions((prev) => prev.map((x, j) => (j === i ? r : x)));
  };

  const save = async () => {
    const ok = await run("Resolve conflict", () => api.writeResolvedFile(store.repo!.id, p.path, result()).then(() => true));
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
            {unresolved() ? `${unresolved()} conflict${unresolved() === 1 ? "" : "s"} left` : "All conflicts resolved"}
          </span>
          <button class="btn" onClick={closeDialog}>
            Cancel
          </button>
          <button class="btn primary" disabled={unresolved() > 0} onClick={save}>
            Mark as resolved
          </button>
        </>
      }
    >
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
                </div>
              )
            }
          </For>
        </Show>
      </div>
    </Dialog>
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
