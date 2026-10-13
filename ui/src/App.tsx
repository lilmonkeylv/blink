import { createEffect, createMemo, createSignal, For, Match, on, onCleanup, onMount, Show, Switch } from "solid-js";
import { api, onChecksChanged, onLaunch, onRepoChanged, onSyncProgress, onUpdateAvailable } from "./api";
import { ChangesDiff, ChangesList } from "./components/ChangesPane";
import { ContextMenuHost } from "./components/ContextMenu";
import { Dialog } from "./components/Dialog";
import { Dialogs } from "./components/Dialogs";
import { CommitView, HistoryList } from "./components/HistoryPane";
import { GearIcon } from "./components/Icons";
import { appMenu, doSync, pickAndAddRepository, Toolbar } from "./components/Toolbar";
import { fuzzyFilter } from "./lib/fuzzy";
import {
  applyRepoView,
  applyTheme,
  bootstrap,
  dismiss,
  handleLaunches,
  offerUpdate,
  showWhatsNew,
  openRepo,
  refreshChecks,
  refreshRepo,
  reportError,
  run,
  setStore,
  store,
  undoLastOperation,
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
            <a class="muted" onClick={() => dismiss(b.id)}>
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
          { label: "New branch…", hint: "Ctrl+Shift+N", run: () => setStore("dialog", { type: "newBranch" }) },
          { label: "Merge into current branch…", run: () => setStore("dialog", { type: "merge" }) },
          { label: "Rebase current branch…", run: () => setStore("dialog", { type: "rebase" }) },
          { label: "Stash all changes", hint: "Ctrl+Shift+S", run: () => run("Stash", () => api.stash(store.repo!.id)) },
          { label: "Undo last Git operation", hint: "Ctrl+Z", run: undoLastOperation },
          { label: "Search history", hint: "Ctrl+F", run: focusSearch },
          { label: "Refresh", hint: "F5", run: refreshRepo },
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
    const mod = e.ctrlKey || e.metaKey;
    const key = e.key.toLowerCase();
    const repo = store.repo;
    if (e.key === "F12" && import.meta.env.DEV) {
      e.preventDefault();
      api.openDevtools().catch(() => {});
      return;
    }
    if (e.key === "F5") {
      e.preventDefault();
      refreshRepo();
      return;
    }
    if (!mod) return;
    const hit = (fn: () => void) => {
      e.preventDefault();
      fn();
    };
    const id = repo?.id;
    const call = (fn: (id: string) => Promise<unknown>) => () => id && fn(id).catch(reportError);
    if (key === "k") hit(() => setPalette(true));
    else if (key === "1") hit(() => setStore("tab", "changes"));
    else if (key === "2") hit(() => setStore("tab", "history"));
    else if (key === "z" && !e.shiftKey && repo && !inTextField(e) && !store.dialog) hit(undoLastOperation);
    else if (key === "f" && e.shiftKey && repo) hit(call((i) => api.reveal(i)));
    else if (key === "f" && repo) hit(focusSearch);
    else if (key === "r" && repo) hit(call(api.createPullRequest));
    else if (key === "a" && e.shiftKey && repo) hit(call((i) => api.openInEditor(i)));
    else if (key === "`" && repo) hit(call((i) => api.openInShell(i)));
    else if (key === "g" && e.shiftKey && repo) hit(call(api.openOnGithub));
    else if (key === "l" && e.shiftKey && repo && store.selectedPath) hit(() => setStore("dialog", { type: "blame", path: store.selectedPath! }));
    else if (key === "o" && e.shiftKey) hit(() => setStore("dialog", { type: "clone" }));
    else if (key === "t" && e.shiftKey) hit(() => doSync("fetch"));
    else if (key === "t") hit(() => setStore("popover", store.popover === "repos" ? null : "repos"));
    else if (key === "b" && repo) hit(() => setStore("popover", store.popover === "branches" ? null : "branches"));
    else if (key === "n" && e.shiftKey && repo) hit(() => setStore("dialog", { type: "newBranch" }));
    else if (key === "n") hit(() => setStore("dialog", { type: "createRepo" }));
    else if (key === "o") hit(pickAndAddRepository);
    else if (key === "p" && e.shiftKey && repo) hit(() => doSync("pull"));
    else if (key === "p" && repo) hit(() => doSync("push"));
    else if (key === "s" && e.shiftKey && repo) hit(() => run("Stash", () => api.stash(repo.id)));
    else if (key === ",") hit(() => setStore("dialog", { type: "settings" }));
  };

  const listeners = Promise.all([
    onRepoChanged(applyRepoView),
    onSyncProgress((e) => {
      if (store.sync && store.repo?.id === e.repo) setStore("sync", "progress", e.progress);
    }),
    onChecksChanged((e) => {
      if (store.repo?.id === e.repo) setStore("checks", e.summary);
    }),
    onLaunch(handleLaunches),
    onUpdateAvailable(offerUpdate),
  ]);
  onCleanup(() => {
    window.removeEventListener("keydown", onKey);
    listeners.then((fns) => fns.forEach((u) => u()));
  });

  createEffect(
    on(
      () => [store.repo?.id, store.repo?.head?.type === "branch" ? store.repo.head.id : null, store.account?.login] as const,
      () => refreshChecks(),
    ),
  );

  onMount(async () => {
    window.addEventListener("keydown", onKey);
    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyTheme);
    try {
      await bootstrap();
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
