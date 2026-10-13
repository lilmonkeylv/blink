import { getCurrentWindow } from "@tauri-apps/api/window";
import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import { api, onChecksChanged, onLaunch, onRepoChanged, onSharedChanged, onSyncProgress, onUpdateAvailable } from "./api";
import { ChangesDiff, ChangesList } from "./components/ChangesPane";
import { ContextMenuHost } from "./components/ContextMenu";
import { Dialog } from "./components/Dialog";
import { Dialogs } from "./components/Dialogs";
import { CommitView, HistoryList } from "./components/HistoryPane";
import { GearIcon } from "./components/Icons";
import { TabStrip } from "./components/TabStrip";
import { applySidebarWidth, clampSidebar, saveRepoLayout } from "./lib/layout";
import { appMenu, doSync, pickAndAddRepository, Toolbar } from "./components/Toolbar";
import { fuzzyFilter } from "./lib/fuzzy";
import { comboFromEvent } from "./lib/keymap";
import {
  applyRepoView,
  applyTheme,
  bootstrap,
  closeRepoTab,
  cycleRepoTab,
  dismiss,
  handleLaunches,
  keymap,
  loadKeymap,
  offerUpdate,
  openRepo,
  pickNewTab,
  refreshChecks,
  refreshCommitWarnings,
  refreshRepo,
  reloadSharedState,
  reportError,
  run,
  setStore,
  showWhatsNew,
  store,
  undoLastOperation,
  windowRepo,
} from "./store";

function Banners() {
  return (
    <div class="banners">
      <For each={store.banners}>
        {(b) => (
          <div class={`banner ${b.tone}`} role={b.tone === "error" ? "alert" : "status"}>
            <div class="msg">
              {b.message}
              <Show when={b.detail}>
                <details>
                  <summary class="muted">Show Git output</summary>
                  <pre>{b.detail}</pre>
                </details>
              </Show>
            </div>
            <Show when={b.action}>
              {(a) => (
                <a
                  onClick={() => {
                    dismiss(b.id);
                    a().run();
                  }}
                >
                  {a().label}
                </a>
              )}
            </Show>
            <a
              class="muted"
              onClick={() => {
                b.onDismiss?.();
                dismiss(b.id);
              }}
            >
              Dismiss
            </a>
          </div>
        )}
      </For>
    </div>
  );
}

function Welcome() {
  return (
    <div class="welcome">
      <div class="welcome-card">
        <h1>Welcome to Blink</h1>
        <span class="muted">A fast Git client for GitHub. Add a repository to get started.</span>
        <Show when={!store.gitFound}>
          <div class="warning">Git was not found on your PATH. Install Git 2.40 or newer, then restart Blink.</div>
        </Show>
        <div class="actions">
          <button class="btn primary" onClick={() => setStore("dialog", { type: "clone" })}>
            Clone a repository from the internet…
          </button>
          <button class="btn" onClick={pickAndAddRepository}>
            Add an existing repository from your computer…
          </button>
          <button class="btn" onClick={() => setStore("dialog", { type: "createRepo" })}>
            Create a new repository…
          </button>
          <Show when={!store.repositories.length}>
            <button class="btn" onClick={() => setStore("dialog", { type: "import" })}>
              Import repositories from GitHub Desktop or a folder…
            </button>
          </Show>
          <Show when={!store.account}>
            <button class="btn" onClick={() => setStore("dialog", { type: "signIn" })}>
              Sign in to GitHub.com
            </button>
          </Show>
        </div>
        <span class="muted" style={{ "font-size": "12px" }}>
          Tip: drag a folder onto this window to add it, or run <span class="kbd">blink .</span> in a terminal.
        </span>
      </div>
    </div>
  );
}

function Loading(p: { label: string }) {
  return (
    <div class="empty">
      <div class="loading">
        <span class="spinner large" />
        <span class="muted">{p.label}…</span>
      </div>
    </div>
  );
}

function SidebarResizer() {
  const start = (e: PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
    let width = 0;
    const move = (ev: PointerEvent) => {
      width = clampSidebar(ev.clientX);
      applySidebarWidth(width);
    };
    const done = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", done);
      document.documentElement.classList.remove("resizing");
      const id = store.repo?.id;
      if (id && width) saveRepoLayout(id, { sidebar: width });
    };
    document.documentElement.classList.add("resizing");
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", done);
  };
  return <div class="sidebar-resizer" onPointerDown={start} title="Drag to resize" />;
}

function Sidebar() {
  const changes = () => store.repo?.files.length ?? 0;
  return (
    <div class="sidebar">
      <div class="tabs" role="tablist">
        <button class="tab" classList={{ active: store.tab === "changes" }} onClick={() => setStore("tab", "changes")} title="Ctrl+1">
          Changes{" "}
          <Show when={changes()}>
            <span class="badge">{changes()}</span>
          </Show>
        </button>
        <button class="tab" classList={{ active: store.tab === "history" }} onClick={() => setStore("tab", "history")} title="Ctrl+2">
          History
        </button>
      </div>
      <Switch>
        <Match when={store.tab === "changes"}>
          <ChangesList />
        </Match>
        <Match when={store.tab === "history"}>
          <HistoryList />
        </Match>
      </Switch>
      <div class="list-header" style={{ "border-top": "1px solid var(--border)", "border-bottom": 0 }}>
        <Show when={store.account} fallback={<a onClick={() => setStore("dialog", { type: "signIn" })}>Sign in to GitHub.com</a>}>
          {(a) => <span class="muted">@{a().login}</span>}
        </Show>
        <span style={{ flex: 1 }} />
        <button class="icon-button" title="Settings (Ctrl+,)" onClick={() => setStore("dialog", { type: "settings" })}>
          <GearIcon />
        </button>
      </div>
    </div>
  );
}

interface PaletteItem {
  label: string;
  hint?: string;
  run: () => void;
}

function Palette(p: { onClose: () => void }) {
  const [query, setQuery] = createSignal("");
  const [active, setActive] = createSignal(0);
  const items = createMemo<PaletteItem[]>(() => {
    const actions = appMenu().filter((m): m is Exclude<typeof m, "separator"> => m !== "separator" && !m.disabled);
    const extra: PaletteItem[] = store.repo
      ? [
          { label: "New branch…", hint: keymap().keys["branch.new"], run: () => setStore("dialog", { type: "newBranch" }) },
          { label: "Merge into current branch…", run: () => setStore("dialog", { type: "merge" }) },
          { label: "Rebase current branch…", run: () => setStore("dialog", { type: "rebase" }) },
          { label: "Stash all changes", hint: keymap().keys["stash.all"], run: () => run("Stash", () => api.stash(store.repo!.id)) },
          { label: "Undo last Git operation", hint: keymap().keys["undo"], run: undoLastOperation },
          { label: "Search history", hint: keymap().keys["history.search"], run: focusSearch },
          { label: "Refresh", hint: keymap().keys["refresh"], run: refreshRepo },
        ]
      : [];
    const repos: PaletteItem[] = store.repositories.map((r) => ({ label: `Open ${r.name}`, hint: r.owner ?? undefined, run: () => openRepo(r.path) }));
    return fuzzyFilter([...extra, ...actions.map((a) => ({ label: a.label, run: a.run })), ...repos], query(), (i) => i.label);
  });

  const choose = (item?: PaletteItem) => {
    if (!item) return;
    p.onClose();
    item.run();
  };

  return (
    <Dialog title="Command palette" onClose={p.onClose} class="palette">
      <input
        class="input"
        autofocus
        placeholder="Type a command or repository"
        value={query()}
        onInput={(e) => {
          setQuery(e.currentTarget.value);
          setActive(0);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setActive(Math.min(active() + 1, items().length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive(Math.max(active() - 1, 0));
          } else if (e.key === "Enter") {
            choose(items()[active()]);
          }
        }}
      />
      <div class="list">
        <For each={items().slice(0, 60)}>
          {(item, i) => (
            <div class="static-row" classList={{ active: i() === active() }} onMouseEnter={() => setActive(i())} onClick={() => choose(item)}>
              <span class="grow">{item.label}</span>
              <Show when={item.hint}>
                <span class="kbd">{item.hint}</span>
              </Show>
            </div>
          )}
        </For>
      </div>
    </Dialog>
  );
}

function focusSearch() {
  setStore("tab", "history");
  setTimeout(() => document.getElementById("history-search")?.focus(), 30);
}

function inTextField(e: KeyboardEvent) {
  const el = e.target as HTMLElement;
  return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
}

export function App() {
  const [palette, setPalette] = createSignal(false);

  const onKey = (e: KeyboardEvent) => {
    const repo = store.repo;
    if (e.key === "F12" && import.meta.env.DEV) {
      e.preventDefault();
      api.openDevtools().catch(() => {});
      return;
    }
    const action = keymap().byCombo.get(comboFromEvent(e));
    if (!action) return;
    const id = repo?.id;
    const call = (fn: (id: string) => Promise<unknown>) => () => id && fn(id).catch(reportError);
    const manyTabs = store.tabs.length > 1;
    const actions: Record<string, { when?: boolean; run: () => void }> = {
      "view.changes": { run: () => setStore("tab", "changes") },
      "view.history": { run: () => setStore("tab", "history") },
      "repo.list": { run: () => setStore("popover", store.popover === "repos" ? null : "repos") },
      "tab.new": { run: pickNewTab },
      "tab.next": { when: manyTabs, run: () => cycleRepoTab(1) },
      "tab.previous": { when: manyTabs, run: () => cycleRepoTab(-1) },
      "tab.nextPage": { when: manyTabs, run: () => cycleRepoTab(1) },
      "tab.previousPage": { when: manyTabs, run: () => cycleRepoTab(-1) },
      "tab.close": { when: manyTabs && !!repo, run: () => closeRepoTab(repo!.id) },
      "branch.list": { when: !!repo, run: () => setStore("popover", store.popover === "branches" ? null : "branches") },
      "branch.new": { when: !!repo, run: () => setStore("dialog", { type: "newBranch" }) },
      "repo.add": { run: pickAndAddRepository },
      "repo.clone": { run: () => setStore("dialog", { type: "clone" }) },
      "repo.create": { run: () => setStore("dialog", { type: "createRepo" }) },
      "sync.push": { when: !!repo, run: () => doSync("push") },
      "sync.pull": { when: !!repo, run: () => doSync("pull") },
      "sync.fetch": { run: () => doSync("fetch") },
      undo: { when: !!repo && !inTextField(e) && !store.dialog, run: undoLastOperation },
      "history.search": { when: !!repo, run: focusSearch },
      "pr.create": { when: !!repo, run: call(api.createPullRequest) },
      "open.editor": { when: !!repo, run: call((i) => api.openInEditor(i)) },
      "open.shell": { when: !!repo, run: call((i) => api.openInShell(i)) },
      "open.files": { when: !!repo, run: call((i) => api.reveal(i)) },
      "open.github": { when: !!repo, run: call(api.openOnGithub) },
      "file.blame": { when: !!repo && !!store.selectedPath, run: () => setStore("dialog", { type: "blame", path: store.selectedPath! }) },
      "stash.all": { when: !!repo, run: () => run("Stash", () => api.stash(repo!.id)) },
      palette: { run: () => setPalette(true) },
      settings: { run: () => setStore("dialog", { type: "settings" }) },
      refresh: { run: refreshRepo },
    };
    const target = actions[action];
    if (!target || target.when === false) return;
    e.preventDefault();
    target.run();
  };

  const listeners = Promise.all([
    onRepoChanged(applyRepoView),
    onSharedChanged(() => reloadSharedState().catch(reportError)),
    onSyncProgress((e) => {
      if (store.sync && store.repo?.id === e.repo) setStore("sync", "progress", e.progress);
    }),
    onChecksChanged((e) => {
      if (store.repo?.id === e.repo) setStore("checks", e.summary);
    }),
    ...(windowRepo ? [] : [onLaunch(handleLaunches), onUpdateAvailable(offerUpdate)]),
  ]);
  onCleanup(() => {
    window.removeEventListener("keydown", onKey);
    listeners.then((fns) => fns.forEach((u) => u()));
  });

  createEffect(() => {
    const id = store.repo?.id;
    if (id) saveRepoLayout(id, { tab: store.tab, selectedPath: store.selectedPath });
  });

  createEffect(() => {
    const title = store.repo ? `${store.repo.name} - Blink` : "Blink";
    if ("__TAURI_INTERNALS__" in window) getCurrentWindow().setTitle(title).catch(() => {});
  });

  createEffect(
    on(
      () => [store.repo?.id, store.repo?.head?.type === "branch" ? store.repo.head.id : null, store.account?.login] as const,
      () => {
        refreshChecks();
        refreshCommitWarnings();
      },
    ),
  );

  onMount(async () => {
    window.addEventListener("keydown", onKey);
    window.addEventListener("focus", loadKeymap);
    loadKeymap();
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyTheme);
    try {
      await bootstrap();
      if (windowRepo) return;
      await handleLaunches();
      if (!store.dialog) await showWhatsNew();
    } catch (e) {
      reportError(e, "Startup failed");
      setStore("ready", true);
    }
  });

  return (
    <div
      class="app"
      style={{ position: "relative" }}
      onMouseDown={(e) => store.popover && !(e.target as HTMLElement).closest(".popover, .toolbar-button, .menu") && setStore("popover", null)}
    >
      <Toolbar />
      <TabStrip />
      <Banners />
      <Show when={store.ready} fallback={<Loading label="Starting Blink" />}>
        <Show when={store.repo} fallback={store.opening ? <Loading label="Opening repository" /> : <Welcome />}>
          <div
            class="main"
            style={{
              "--diff-font-size": `${store.settings?.appearance.diffFontSize ?? 12}px`,
              "--tab-size": `${store.settings?.appearance.tabSize ?? 8}`,
            }}
          >
            <Sidebar />
            <SidebarResizer />
            <div class="content">
              <Switch>
                <Match when={store.tab === "changes"}>
                  <ChangesDiff />
                </Match>
                <Match when={store.tab === "history"}>
                  <CommitView />
                </Match>
              </Switch>
            </div>
          </div>
        </Show>
      </Show>
      <Dialogs />
      <Show when={palette()}>
        <Palette onClose={() => setPalette(false)} />
      </Show>
      <ContextMenuHost />
    </div>
  );
}
