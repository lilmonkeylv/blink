import { getCurrentWindow } from "@tauri-apps/api/window";
import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import { api, onChecksChanged, onHookOutput, onLaunch, onRepoChanged, onSharedChanged, onSyncProgress, onTrayAction, onUpdateAvailable, type TrayAction } from "./api";
import { ChangesDiff, ChangesList } from "./components/ChangesPane";
import { BisectBar } from "./components/BisectBar";
import { ContextMenuHost, menuActions } from "./components/ContextMenu";
import { HookPanel } from "./components/HookPanel";
import { PromptHost } from "./components/Prompt";
import { Dialog, dialogOpen } from "./components/Dialog";
import { Dialogs } from "./components/Dialogs";
import { Loading } from "./components/Loading";
import { CommitView, HistoryList } from "./components/HistoryPane";
import { TabStrip } from "./components/TabStrip";
import { Toasts } from "./components/Toasts";
import { TooltipHost } from "./components/Tooltip";
import { applySidebarWidth, clampSidebar, reclampSidebar, saveRepoLayout } from "./lib/layout";
import { appMenu, doSync, pickAndAddRepository, Toolbar } from "./components/Toolbar";
import { fuzzyFilter } from "./lib/fuzzy";
import { applyHookEvent } from "./lib/hooks";
import { comboFromEvent } from "./lib/keymap";
import { cycleTab } from "./lib/tabs";
import {
  applyRepoView,
  applyTheme,
  bootstrap,
  checkForUpdates,
  closeRepoTab,
  cycleRepoTab,
  handleLaunches,
  keymap,
  loadKeymap,
  offerUpdate,
  openRepo,
  pickNewTab,
  refreshBisect,
  refreshChecks,
  refreshCommitWarnings,
  refreshLfs,
  refreshSubmodules,
  refreshRepo,
  reloadSharedState,
  reportError,
  run,
  setStore,
  shortcut,
  showWhatsNew,
  store,
  undoLastOperation,
  windowRepo,
} from "./store";
import { nativeWindow } from "./components/WindowControls";

function runTrayAction(action: TrayAction) {
  if (action === "fetchAll") setStore("dialog", { type: "fetchAll" });
  else void checkForUpdates(true);
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
  return <div class="sidebar-resizer" onPointerDown={start} data-tooltip="Drag to resize" />;
}

function Sidebar() {
  const changes = () => store.repo?.files.length ?? 0;
  return (
    <div class="sidebar">
      <div class="tabs sidebar-tabs" classList={{ history: store.tab === "history" }} role="tablist">
        <button class="tab" classList={{ active: store.tab === "changes" }} onClick={() => setStore("tab", "changes")} data-tooltip="Changes" data-shortcut={shortcut("view.changes")}>
          Changes{" "}
          <Show when={changes()}>
            <span class="badge">{changes()}</span>
          </Show>
        </button>
        <button class="tab" classList={{ active: store.tab === "history" }} onClick={() => setStore("tab", "history")} data-tooltip="History" data-shortcut={shortcut("view.history")}>
          History
        </button>
      </div>
      <BisectBar />
      <Switch>
        <Match when={store.tab === "changes"}>
          <ChangesList />
        </Match>
        <Match when={store.tab === "history"}>
          <HistoryList />
        </Match>
      </Switch>
      <HookPanel />
      <Show when={!store.account}>
        <div class="list-header" style={{ "border-top": "1px solid var(--border)", "border-bottom": 0 }}>
          <a onClick={() => setStore("dialog", { type: "signIn" })}>Sign in to GitHub.com</a>
        </div>
      </Show>
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
    const actions = menuActions(appMenu());
    const extra: PaletteItem[] = store.repo
      ? [
          { label: "New branch…", hint: shortcut("branch.new"), run: () => setStore("dialog", { type: "newBranch" }) },
          { label: "Merge into current branch…", run: () => setStore("dialog", { type: "merge" }) },
          { label: "Rebase current branch…", run: () => setStore("dialog", { type: "rebase" }) },
          { label: "Stash all changes", hint: shortcut("stash.all"), run: () => run("Stash", () => api.stash(store.repo!.id)) },
          { label: "Undo last Git operation", hint: shortcut("undo"), run: undoLastOperation },
          { label: "Search history", hint: shortcut("history.search"), run: focusSearch },
          { label: "Refresh", hint: shortcut("refresh"), run: refreshRepo },
        ]
      : [];
    const repos: PaletteItem[] = store.repositories.map((r) => ({ label: `Open ${r.name}`, hint: r.owner ?? undefined, run: () => openRepo(r.path) }));
    return fuzzyFilter([...extra, ...actions.map((a) => ({ label: a.label, hint: a.hint, run: a.run })), ...repos], query(), (i) => i.label);
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

const REGIONS = [".toolbar", ".sidebar .list", ".commit-box", ".content"];
const FOCUSABLE = "[tabindex='0'], button:not(:disabled), input:not(:disabled), textarea:not(:disabled)";

function focusRegion(step: number) {
  const present = REGIONS.filter((s) => document.querySelector(s));
  const current = present.find((s) => document.activeElement?.closest(s)) ?? null;
  const next = cycleTab(present, current, step);
  const region = next ? document.querySelector<HTMLElement>(next) : null;
  if (!region) return;
  const target = region.matches(FOCUSABLE) ? region : (region.querySelector<HTMLElement>(".list") ?? region.querySelector<HTMLElement>(FOCUSABLE));
  target?.focus();
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
    if (e.key === "F6" && store.settings?.accessibility.keyboardNavigation && !store.dialog) {
      e.preventDefault();
      focusRegion(e.shiftKey ? -1 : 1);
      return;
    }
    const action = keymap().byCombo.get(comboFromEvent(e));
    if (!action) return;
    if (action === "palette") {
      e.preventDefault();
      setPalette(!palette());
      return;
    }
    if (dialogOpen()) return;
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
      undo: { when: !!repo && !inTextField(e), run: undoLastOperation },
      "history.search": { when: !!repo, run: focusSearch },
      "pr.create": { when: !!repo, run: call(api.createPullRequest) },
      "open.editor": { when: !!repo, run: call((i) => api.openInEditor(i)) },
      "open.shell": { when: !!repo, run: call((i) => api.openInShell(i)) },
      "open.files": { when: !!repo, run: call((i) => api.reveal(i)) },
      "open.github": { when: !!repo, run: call(api.openOnGithub) },
      "file.blame": { when: !!repo && !!store.selectedPath, run: () => setStore("dialog", { type: "blame", path: store.selectedPath! }) },
      "stash.all": { when: !!repo, run: () => run("Stash", () => api.stash(repo!.id)) },
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
    onHookOutput((e) => {
      if (store.repo?.id !== e.repo) return;
      if (e.event.type === "started") setStore("hookRetry", null);
      setStore("hookRun", applyHookEvent(store.hookRun, e.repo, e.action, e.event));
    }),
    onChecksChanged((e) => {
      if (store.repo?.id === e.repo) setStore("checks", e.summary);
    }),
    ...(windowRepo ? [] : [onLaunch(handleLaunches), onUpdateAvailable(offerUpdate), onTrayAction(runTrayAction)]),
  ]);
  onCleanup(() => {
    window.removeEventListener("keydown", onKey);
    window.removeEventListener("resize", reclampSidebar);
    listeners.then((fns) => fns.forEach((u) => u()));
  });

  createEffect(() => {
    const id = store.repo?.id;
    if (id) saveRepoLayout(id, { tab: store.tab, selectedPath: store.selectedPath });
  });

  createEffect(() => {
    const title = store.repo ? `${store.repo.name} - Blink` : "Blink";
    if (nativeWindow) getCurrentWindow().setTitle(title).catch(() => {});
  });

  createEffect(
    on(
      () => [store.repo?.id, store.repo?.head?.type === "branch" ? store.repo.head.id : null, store.account?.login] as const,
      () => {
        refreshChecks();
        refreshCommitWarnings();
        refreshSubmodules();
        refreshLfs();
      },
    ),
  );

  createEffect(on(() => [store.repo?.id, store.repo?.bisecting, store.repo?.generation] as const, refreshBisect));

  onMount(async () => {
    window.addEventListener("keydown", onKey);
    window.addEventListener("focus", loadKeymap);
    window.addEventListener("resize", reclampSidebar);
    loadKeymap();
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => applyTheme());
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
      <PromptHost />
      <ContextMenuHost />
      <Toasts />
      <TooltipHost />
    </div>
  );
}
