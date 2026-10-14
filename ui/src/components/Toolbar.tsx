import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { createMemo, createResource, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { api, type Branch, type PullRequest, type RepoRecord, type SyncAction } from "../api";
import { fuzzyFilter } from "../lib/fuzzy";
import { relativeTime } from "../lib/time";
import {
  addRepository,
  checkForUpdates,
  closeRepoTab,
  currentBranch,
  keymap,
  notify,
  openInNewTab,
  openInNewWindow,
  openRepo,
  reloadRepositories,
  reportError,
  run,
  setStore,
  showWhatsNew,
  store,
  undoLastOperation,
} from "../store";
import { openContextMenu, type MenuItem } from "./ContextMenu";
import { customTitleBar, WindowControls } from "./WindowControls";
import { DRAG_TYPE } from "./HistoryPane";
import { BranchIcon, CheckIcon, ChevronDown, DownIcon, MenuIcon, PlusIcon, PullRequestIcon, RepoIcon, SyncIcon, UpIcon } from "./Icons";
import { Loading } from "./Loading";
import { ask, choose, message } from "./Prompt";
import { transferPercent } from "../lib/progress";

export async function pickAndAddRepository() {
  const dir = await openDialog({ directory: true, multiple: false, title: "Add local repository" });
  if (typeof dir === "string") await addRepository(dir);
}

function usePopoverDismiss(close: () => void) {
  const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
  onMount(() => window.addEventListener("keydown", onKey));
  onCleanup(() => window.removeEventListener("keydown", onKey));
}

const addMenu = (): MenuItem[] => [
  { label: "Add existing repository…", hint: keymap().keys["repo.add"], run: () => pickAndAddRepository() },
  { label: "Clone repository…", hint: keymap().keys["repo.clone"], run: () => setStore("dialog", { type: "clone" }) },
  { label: "Create new repository…", hint: keymap().keys["repo.create"], run: () => setStore("dialog", { type: "createRepo" }) },
  "separator",
  { label: "Import repositories from a folder…", run: () => setStore("dialog", { type: "import" }) },
];

function RepoPopover() {
  const [query, setQuery] = createSignal("");
  const [active, setActive] = createSignal(-1);
  usePopoverDismiss(() => setStore("popover", null));
  onCleanup(() => setStore("pickingNewTab", false));

  const filtered = createMemo(() => fuzzyFilter(store.repositories, query(), (r) => `${r.owner ?? ""}/${r.name}`));
  const groups = createMemo(() => {
    const map = new Map<string, RepoRecord[]>();
    for (const r of filtered()) {
      const key = query() ? "Results" : (r.owner ?? "Other");
      map.set(key, [...(map.get(key) ?? []), r]);
    }
    return [...map.entries()];
  });
  const flat = () => groups().flatMap(([, rs]) => rs);

  const remove = async (r: RepoRecord) => {
    if (store.settings?.prompts.removeRepository !== false) {
      const ok = await ask(`Remove ${r.name} from Blink? The folder on disk is not deleted.`, { title: "Remove repository", okLabel: "Remove" });
      if (!ok) return;
    }
    await run("Remove repository", () => api.removeRepository(r.path));
    if (store.tabs.includes(r.path)) closeRepoTab(r.path);
    else if (store.repo?.id === r.path) setStore("repo", null);
    await reloadRepositories();
  };

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActive(Math.min(active() + 1, flat().length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setActive(Math.max(active() - 1, 0));
    } else if (e.key === "Enter") {
      const target = active() < 0 && query() ? flat()[0] : flat()[active()];
      if (target) openRepo(target.path, e.ctrlKey || e.metaKey || store.pickingNewTab);
    }
  };

  return (
    <div class="popover" style={{ left: "0", width: "var(--sidebar)", top: "var(--toolbar)" }}>
      <div class="filter">
        <input
          class="input"
          placeholder={store.pickingNewTab ? "Open in a new tab" : "Filter"}
          ref={(el) => setTimeout(() => el.focus())}
          value={query()}
          onInput={(e) => {
            setQuery(e.currentTarget.value);
            setActive(e.currentTarget.value ? 0 : -1);
          }}
          onKeyDown={onKey}
        />
        <button class="btn" onClick={(e) => openContextMenu(e, addMenu())}>
          <PlusIcon /> Add
        </button>
      </div>
      <div class="list" style={{ position: "relative" }}>
        <For each={groups()} fallback={<div class="empty">No repositories</div>}>
          {([group, repos]) => (
            <>
              <div class="group-title">{group}</div>
              <For each={repos}>
                {(r) => (
                  <div
                    class="static-row"
                    classList={{ current: store.repo?.id === r.path, active: flat()[active()] === r }}
                    onClick={(e) => openRepo(r.path, e.ctrlKey || e.metaKey || store.pickingNewTab)}
                    onAuxClick={(e) => e.button === 1 && openInNewTab(r.path)}
                    onContextMenu={(e) =>
                      openContextMenu(e, [
                        { label: "Open in new tab", run: () => openInNewTab(r.path) },
                        { label: "Open in new window", run: () => openInNewWindow(r.path) },
                        { label: "Copy repository path", run: () => navigator.clipboard.writeText(r.path) },
                        "separator",
                        { label: "Remove…", run: () => remove(r) },
                      ])
                    }
                    title={r.path}
                  >
                    <RepoIcon />
                    <span class="grow">{r.name}</span>
                    <Show when={store.repo?.id === r.path}>
                      <CheckIcon class="current-mark" />
                    </Show>
                  </div>
                )}
              </For>
            </>
          )}
        </For>
      </div>
    </div>
  );
}

function PullRequestList(p: { query: string }) {
  const [prs, { refetch }] = createResource(
    () => store.repo?.id,
    (id) => api.pullRequests(id),
  );
  const list = createMemo(() => fuzzyFilter(prs() ?? [], p.query, (pr) => `#${pr.number} ${pr.title} ${pr.user.login}`));
  const checkout = async (pr: PullRequest) => {
    setStore("popover", null);
    await run(`Check out #${pr.number}`, () => api.checkoutPullRequest(store.repo!.id, pr.number, pr.head.ref));
  };
  return (
    <Show
      when={(prs.error as { kind?: string } | undefined)?.kind !== "signedOut"}
      fallback={
        <div class="empty">
          <div>
            <p>{String((prs.error as { message?: string })?.message ?? "Sign in to see pull requests.")}</p>
            <button class="btn primary" onClick={() => setStore("dialog", { type: "signIn" })}>
              Sign in
            </button>
          </div>
        </div>
      }
    >
      <Show when={!prs.error} fallback={<div class="empty muted">{String((prs.error as { message?: string })?.message ?? "Could not load pull requests")}</div>}>
        <For each={list()} fallback={prs.loading ? <Loading /> : <div class="empty muted">No open pull requests</div>}>
          {(pr) => (
            <div
              class="static-row pr-row"
              classList={{ current: pr.head.ref === currentBranch() }}
              onClick={() => checkout(pr)}
              onContextMenu={(e) =>
                openContextMenu(e, [
                  { label: "Review…", run: () => setStore({ popover: null, dialog: { type: "review", number: pr.number, title: pr.title } }) },
                  { label: "Check out", run: () => checkout(pr) },
                  { label: "Copy link", run: () => navigator.clipboard.writeText(pr.html_url) },
                  { label: "Refresh list", run: () => refetch() },
                ])
              }
              title={pr.title}
            >
              <PullRequestIcon class={pr.draft ? "muted" : "pr-open"} />
              <span class="grow">
                <span class="pr-title">{pr.title}</span>
                <span class="muted small">
                  #{pr.number} by {pr.user.login} · {relativeTime(Date.parse(pr.updated_at) / 1000)}
                  {pr.draft ? " · draft" : ""}
                </span>
              </span>
              <Show when={pr.checks && pr.checks !== "none"}>
                <span class={`dot ${pr.checks}`} title={`Checks: ${pr.checks}`} />
              </Show>
            </div>
          )}
        </For>
      </Show>
    </Show>
  );
}

function BranchPopover() {
  const [query, setQuery] = createSignal("");
  const [tab, setTab] = createSignal<"branches" | "prs">("branches");
  usePopoverDismiss(() => setStore("popover", null));
  const [branches, { refetch }] = createResource(
    () => store.repo?.id,
    (id) => api.branches(id),
  );

  const current = () => currentBranch();
  const groups = createMemo(() => {
    const all = branches() ?? [];
    const locals = all.filter((b) => b.kind === "local");
    const localNames = new Set(locals.map((b) => b.name));
    const remotes = all.filter((b) => b.kind === "remote" && !localNames.has(b.name.slice(b.name.indexOf("/") + 1)));
    const defaultName = ["main", "master", "trunk", "develop"].find((n) => localNames.has(n));
    const filteredLocal = fuzzyFilter(locals, query(), (b) => b.name);
    const filteredRemote = fuzzyFilter(remotes, query(), (b) => b.name);
    if (query()) return [["Branches", [...filteredLocal, ...filteredRemote]] as const];
    const def = filteredLocal.filter((b) => b.name === defaultName);
    const recent = [...filteredLocal].filter((b) => b.name !== defaultName).sort((a, b) => b.lastCommitTime - a.lastCommitTime);
    return [["Default branch", def] as const, ["Other branches", recent] as const, ["Remote branches", filteredRemote] as const].filter(
      ([, list]) => list.length,
    );
  });

  const checkout = async (b: Branch) => {
    const repo = store.repo;
    if (!repo || b.name === current()) return setStore("popover", null);
    let changes: "bringAlong" | "stashOnCurrent" | undefined;
    if (repo.files.length) {
      const mode = store.settings?.advanced.branchSwitch ?? "ask";
      if (mode === "stash-on-current") changes = "stashOnCurrent";
      else if (mode === "bring-changes") changes = "bringAlong";
      else {
        const picked = await choose(
          "Switch branch",
          `You have changes on this branch. Leave them on ${current() ?? "this branch"} (stash) or bring them to ${b.name}?`,
          [
            { label: `Leave on ${current() ?? "branch"}`, value: "stashOnCurrent" as const },
            { label: `Bring to ${b.name}`, value: "bringAlong" as const, primary: true },
          ],
        );
        if (!picked) return;
        changes = picked;
      }
    }
    setStore("popover", null);
    await run("Switch branch", () => api.checkout(repo.id, b.name, b.kind, changes));
  };

  const del = async (b: Branch) => {
    const repo = store.repo!;
    const includeRemote = b.upstream
      ? await ask(`Also delete ${b.upstream} on the remote?`, { title: `Delete ${b.name}`, okLabel: "Delete remote too", cancelLabel: "Local only" })
      : false;
    const ok = await ask(`Delete branch ${b.name}? You can undo this with Ctrl+Z.`, { title: "Delete branch", kind: "warning", okLabel: "Delete" });
    if (!ok) return;
    try {
      await api.deleteBranch(repo.id, b.name, false, includeRemote);
      notify("success", `Deleted ${b.name}`, { action: { label: "Undo", run: undoLastOperation } });
    } catch {
      const force = await ask(`${b.name} has commits that are not merged. Delete it anyway?`, { title: "Delete branch", kind: "warning", okLabel: "Force delete" });
      if (force) await run("Delete branch", () => api.deleteBranch(repo.id, b.name, true, includeRemote));
    }
    refetch();
  };

  const deleteRemote = async (b: Branch) => {
    const repo = store.repo!;
    const ok = await ask(`Delete ${b.name} on the remote? Anyone else using it will lose the branch too.`, {
      title: "Delete remote branch",
      kind: "warning",
      okLabel: "Delete",
    });
    if (!ok) return;
    const done = await run("Delete remote branch", () => api.deleteRemoteBranch(repo.id, b.name).then(() => true));
    if (done) notify("success", `Deleted ${b.name}`);
    refetch();
  };

  const prune = async () => {
    const repo = store.repo!;
    const pruned = await run("Prune remote branches", () => api.pruneRemotes(repo.id));
    if (!pruned) return;
    notify("success", pruned.length ? `Removed ${pruned.length} stale remote branch${pruned.length === 1 ? "" : "es"}` : "No stale remote branches");
    refetch();
  };

  const merge = async (b: Branch, squash = false) => {
    const repo = store.repo!;
    setStore("popover", null);
    const result = await run("Merge", () => api.merge(repo.id, b.name, squash));
    if (result?.conflicted) notify("error", `Merge of ${b.name} has conflicts. Resolve them in the Changes tab.`);
    else if (result) notify("success", `Merged ${b.name} into ${current()}`);
  };

  const localMenu = (b: Branch): MenuItem[] => [
    { label: "Rename…", run: () => setStore("dialog", { type: "renameBranch", name: b.name }) },
    { label: "Copy branch name", run: () => navigator.clipboard.writeText(b.name) },
    "separator",
    { label: `Merge into ${current()}`, run: () => merge(b), disabled: b.name === current() },
    { label: `Squash and merge into ${current()}`, run: () => merge(b, true), disabled: b.name === current() },
    { label: "Compare with current branch", run: () => { setStore({ popover: null, tab: "history", compareWith: b.name }); }, disabled: b.name === current() },
    { label: b.upstream ? `Tracking ${b.upstream}…` : "Track a remote branch…", run: () => setStore({ popover: null, dialog: { type: "setUpstream", branch: b.name, current: b.upstream } }) },
    "separator",
    { label: "Delete…", run: () => del(b), disabled: b.name === current() },
  ];

  const remoteMenu = (b: Branch): MenuItem[] => [
    { label: "Check out as a tracking branch", run: () => checkout(b) },
    { label: "Copy branch name", run: () => navigator.clipboard.writeText(b.name) },
    "separator",
    { label: `Merge into ${current()}`, run: () => merge(b) },
    { label: "Compare with current branch", run: () => setStore({ popover: null, tab: "history", compareWith: b.name }) },
    "separator",
    { label: "Prune stale remote branches", run: prune },
    { label: "Delete on remote…", run: () => deleteRemote(b) },
  ];

  return (
    <div class="popover" style={{ left: "var(--sidebar)", top: "var(--toolbar)", width: "420px" }}>
      <div class="tabs">
        <button class="tab" classList={{ active: tab() === "branches" }} onClick={() => setTab("branches")}>
          Branches
        </button>
        <button class="tab" classList={{ active: tab() === "prs" }} onClick={() => setTab("prs")}>
          Pull requests
        </button>
      </div>
      <div class="filter">
        <input class="input" placeholder="Filter" ref={(el) => setTimeout(() => el.focus())} value={query()} onInput={(e) => setQuery(e.currentTarget.value)} />
        <Show when={tab() === "branches"}>
          <button class="btn" onClick={() => setStore("dialog", { type: "newBranch" })}>
            <PlusIcon /> New branch
          </button>
        </Show>
      </div>
      <div class="list" style={{ position: "relative" }}>
        <Show when={tab() === "branches"} fallback={<PullRequestList query={query()} />}>
          <For each={groups()}>
            {([title, list]) => (
              <>
                <div class="group-title">{title}</div>
                <For each={list}>
                  {(b) => (
                    <div
                      class="static-row"
                      classList={{ current: b.kind === "local" && b.name === current() }}
                      onClick={() => checkout(b)}
                      onDragOver={(e) => e.dataTransfer?.types.includes(DRAG_TYPE) && b.kind === "local" && e.preventDefault()}
                      onDrop={(e) => {
                        const data = e.dataTransfer?.getData(DRAG_TYPE);
                        if (!data) return;
                        e.preventDefault();
                        setStore({ popover: null, dialog: { type: "cherryPick", shas: (JSON.parse(data) as string[]).reverse() } });
                      }}
                      onContextMenu={(e) =>
                        openContextMenu(
                          e,
                          b.kind === "local" ? localMenu(b) : remoteMenu(b),
                        )
                      }
                    >
                      <BranchIcon />
                      <span class="grow">{b.name}</span>
                      <span class="muted" style={{ "font-size": "11px" }}>
                        {b.lastCommitTime ? relativeTime(b.lastCommitTime) : ""}
                      </span>
                      <Show when={b.kind === "local" && b.name === current()}>
                        <CheckIcon class="current-mark" />
                      </Show>
                    </div>
                  )}
                </For>
              </>
            )}
          </For>
        </Show>
      </div>
      <Show when={tab() === "branches"}>
        <div class="branch-actions">
          <button
            class="btn"
            disabled={!current()}
            title={`Pick a branch and merge its commits into ${current() ?? "the current branch"}`}
            onClick={() => setStore("dialog", { type: "merge" })}
          >
            <span>Merge a branch into {current() ?? "current branch"}…</span>
          </button>
          <button
            class="btn"
            disabled={!current()}
            title={`Pick a branch and replay the commits of ${current() ?? "the current branch"} on top of it`}
            onClick={() => setStore("dialog", { type: "rebase" })}
          >
            <span>Rebase {current() ?? "current branch"} onto a branch…</span>
          </button>
        </div>
      </Show>
    </div>
  );
}

function syncState() {
  const repo = store.repo;
  const ab = repo?.aheadBehind;
  if (!repo) return { action: "fetch" as SyncAction, label: "Fetch origin", kind: "fetch" as const };
  if (!repo.hasRemote) return { action: "push" as SyncAction, label: "Publish repository", kind: "publish" as const };
  if (!repo.head || repo.head.type !== "branch") return { action: "fetch" as SyncAction, label: "Fetch origin", kind: "fetch" as const };
  if (!ab) return { action: "push" as SyncAction, label: "Publish branch", kind: "push" as const };
  if (ab.behind > 0) return { action: "pull" as SyncAction, label: "Pull origin", kind: "pull" as const };
  if (ab.ahead > 0) return { action: "push" as SyncAction, label: "Push origin", kind: "push" as const };
  return { action: "fetch" as SyncAction, label: "Fetch origin", kind: "fetch" as const };
}

export async function doSync(action?: SyncAction, force = false) {
  const repo = store.repo;
  if (!repo || store.sync) return;
  if (!action && syncState().kind === "publish") return setStore("dialog", { type: "publish" });
  const act = action ?? syncState().action;
  if (force && store.settings?.prompts.forcePush !== false) {
    const ok = await ask("Force pushing rewrites history on the remote. Others who pulled this branch may lose work.", {
      title: "Force push",
      kind: "warning",
      okLabel: "Force push",
    });
    if (!ok) return;
  }
  setStore("sync", { action: act, progress: null });
  try {
    await api.sync(repo.id, act, force);
  } catch (e) {
    const detail = typeof e === "object" && e && "detail" in e ? String((e as { detail: unknown }).detail ?? "") : "";
    if (act === "push" && /rejected|non-fast-forward|fetch first/i.test(detail)) {
      await message("The remote has commits you don't have. Pull first, then push.", { title: "Push rejected", kind: "warning" });
    } else {
      reportError(e, `${act[0].toUpperCase()}${act.slice(1)} failed`);
    }
  } finally {
    setStore("sync", null);
  }
}

export function appMenu(): MenuItem[] {
  const repo = store.repo;
  const id = repo?.id;
  const r = (fn: (id: string) => Promise<unknown>) => () => id && fn(id).catch(reportError);
  return [
    { label: "Add repository", items: addMenu() },
    {
      label: "Repository",
      items: [
        { label: "Push", hint: keymap().keys["sync.push"], run: () => doSync("push"), disabled: !repo },
        { label: "Pull", hint: keymap().keys["sync.pull"], run: () => doSync("pull"), disabled: !repo },
        { label: "Fetch", hint: keymap().keys["sync.fetch"], run: () => doSync("fetch"), disabled: !repo },
        { label: "Force push (with lease)…", run: () => doSync("push", true), disabled: !repo },
        "separator",
        { label: "Fetch all repositories…", run: () => setStore("dialog", { type: "fetchAll" }) },
        "separator",
        { label: "Stashes…", run: () => setStore("dialog", { type: "stashes" }), disabled: !repo },
        { label: "Worktrees…", run: () => setStore("dialog", { type: "worktrees" }), disabled: !repo },
        { label: "Apply patch…", run: () => setStore("dialog", { type: "applyPatch" }), disabled: !repo },
        { label: "Undo history…", run: () => setStore("dialog", { type: "undoHistory" }), disabled: !repo },
        "separator",
        { label: "Repository settings…", run: () => setStore("dialog", { type: "repoSettings" }), disabled: !repo },
      ],
    },
    {
      label: "Pull request",
      disabled: !repo,
      items: [
        { label: "Preview pull request…", run: () => setStore("dialog", { type: "previewPullRequest" }) },
        { label: "Create pull request", hint: keymap().keys["pr.create"], run: r((i) => api.createPullRequest(i)) },
        { label: "Checks for this branch…", run: () => setStore("dialog", { type: "checks" }) },
        { label: "View on GitHub", hint: keymap().keys["open.github"], run: r(api.openOnGithub) },
      ],
    },
    {
      label: "Open",
      disabled: !repo,
      items: [
        { label: "Open in new window", run: () => id && openInNewWindow(id) },
        { label: "Open in external editor", hint: keymap().keys["open.editor"], run: r((i) => api.openInEditor(i)) },
        { label: "Open in shell", hint: keymap().keys["open.shell"], run: r((i) => api.openInShell(i)) },
        { label: "Show in file manager", hint: keymap().keys["open.files"], run: r((i) => api.reveal(i)) },
      ],
    },
    "separator",
    { label: "Settings", hint: keymap().keys["settings"], run: () => setStore("dialog", { type: "settings" }) },
    { label: "Keyboard shortcuts", run: () => setStore("dialog", { type: "shortcuts" }) },
    "separator",
    { label: "What's new", run: () => showWhatsNew(true) },
    { label: "Check for updates…", run: () => checkForUpdates(true) },
  ];
}

export function Toolbar() {
  const [dropping, setDropping] = createSignal(false);
  const toggle = (p: "repos" | "branches") => setStore("popover", store.popover === p ? null : p);
  const branchLabel = () => {
    const head = store.repo?.head;
    if (!head) return "-";
    if (head.type === "detached") return `Detached at ${head.id.slice(0, 7)}`;
    return head.name;
  };
  const percent = () => transferPercent(store.sync?.progress);
  const syncPhase = createMemo<string>((previous) => {
    const progress = store.sync?.progress;
    if (!store.sync) return "";
    return progress && progress.percent !== null ? `${progress.phase} ${progress.percent}%` : previous;
  }, "");
  const state = () => syncState();
  const syncTooltip = () => (state().action === "push" ? "Push your commits" : state().action === "pull" ? "Pull new commits" : "Check for new commits");
  const syncShortcut = () => keymap().keys[`sync.${state().action}`];

  return (
    <>
      <div class="toolbar" data-tauri-drag-region={customTitleBar || undefined}>
        <button class="toolbar-button repo" classList={{ open: store.popover === "repos" }} onClick={() => toggle("repos")} data-tooltip="Switch repository" data-shortcut={keymap().keys["repo.list"]}>
          <RepoIcon />
          <span class="labels">
            <span class="title">{store.opening ? "Opening repository" : "Current repository"}</span>
            <span class="value">
              {store.opening ? store.opening.split(/[\/]/).pop() : (store.repo?.name ?? "Select a repository")}
            </span>
          </span>
          <Show when={store.opening} fallback={<ChevronDown />}>
            <span class="spinner" aria-label="Loading" />
          </Show>
        </button>
        <button
          class="toolbar-button branch"
          classList={{ open: store.popover === "branches", "drop-on": dropping() }}
          disabled={!store.repo}
          onClick={() => toggle("branches")}
          onDragOver={(e) => {
            if (!e.dataTransfer?.types.includes(DRAG_TYPE)) return;
            e.preventDefault();
            setDropping(true);
            if (store.popover !== "branches") setStore("popover", "branches");
          }}
          onDragLeave={() => setDropping(false)}
          onDrop={() => setDropping(false)}
          data-tooltip="Switch branch"
          data-shortcut={keymap().keys["branch.list"]}
        >
          <BranchIcon />
          <span class="labels">
            <span class="title">Current branch</span>
            <span class="value">{branchLabel()}</span>
          </span>
          <Show when={store.checks && store.checks.state !== "none"}>
            <span
              class={`dot ${store.checks!.state}`}
              data-tooltip={`${store.checks!.failed} failed, ${store.checks!.pending} pending of ${store.checks!.total} checks`}
              onClick={(e) => {
                e.stopPropagation();
                setStore("dialog", { type: "checks" });
              }}
            />
          </Show>
          <ChevronDown />
        </button>
        <button
          class="toolbar-button sync"
          style={{ position: "relative" }}
          disabled={!store.repo || !!store.sync}
          onClick={() => doSync()}
          data-tooltip={state().kind === "publish" ? undefined : syncTooltip()}
          data-shortcut={syncShortcut()}
          onContextMenu={(e) =>
            store.repo &&
            openContextMenu(e, [
              { label: "Fetch", run: () => doSync("fetch") },
              { label: "Pull", run: () => doSync("pull") },
              { label: "Push", run: () => doSync("push") },
              { label: "Push with tags", run: () => store.repo && run("Push", () => api.sync(store.repo!.id, "push", false, true)) },
              { label: "Force push (with lease)…", run: () => doSync("push", true) },
            ])
          }
        >
          {store.sync ? (
            <SyncIcon class="spinning" />
          ) : state().kind === "push" || state().kind === "publish" ? (
            <UpIcon />
          ) : state().kind === "pull" ? (
            <DownIcon />
          ) : (
            <SyncIcon />
          )}
          <span class="labels">
            <span class="value">{store.sync ? `${store.sync.action[0].toUpperCase()}${store.sync.action.slice(1)}ing…` : state().label}</span>
            <span class="title">
              {store.sync
                ? syncPhase()
                : (state().kind === "publish" ? "Publish this repository to GitHub" : !store.repo?.aheadBehind && store.repo?.head?.type === "branch" ? "Not on remote yet" : "")}
            </span>
          </span>
          <Show when={store.repo?.aheadBehind}>
            {(ab) => (
              <>
                <Show when={ab().ahead}>
                  <span class="badge">
                    {ab().ahead}
                    <UpIcon class="tiny" />
                  </span>
                </Show>
                <Show when={ab().behind}>
                  <span class="badge">
                    {ab().behind}
                    <DownIcon class="tiny" />
                  </span>
                </Show>
              </>
            )}
          </Show>
          <Show when={store.sync}>
            <span class="toolbar-progress" classList={{ indeterminate: percent() === null }} style={{ width: percent() === null ? undefined : `${percent()}%` }} />
          </Show>
        </button>
        <span class="toolbar-spacer" data-tauri-drag-region={customTitleBar || undefined} />
        <Show
          when={customTitleBar}
          fallback={
            <button class="toolbar-button app-menu" aria-label="Menu" data-tooltip="Menu" onMouseDown={(e) => e.preventDefault()} onClick={(e) => openContextMenu(e, appMenu())}>
              <MenuIcon />
            </button>
          }
        >
          <WindowControls onMenu={(e) => openContextMenu(e, appMenu())} />
        </Show>
      </div>
      <Show when={store.opening || store.busy}>
        <div class="activity-bar" title={store.busy ?? "Opening repository"} />
      </Show>
      <Show when={store.popover === "repos"}>
        <RepoPopover />
      </Show>
      <Show when={store.popover === "branches" && store.repo}>
        <BranchPopover />
      </Show>
    </>
  );
}
