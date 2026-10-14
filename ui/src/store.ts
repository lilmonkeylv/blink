import { batch, createSignal } from "solid-js";
import { createStore, produce, reconcile as reconcileStore } from "solid-js/store";
import {
  api,
  isCommandError,
  type Account,
  type Bootstrap,
  type ChecksSummary,
  type CommitWarnings,
  type ReleaseNote,
  type FileDiff,
  type LfsLock,
  type LfsStatus,
  type Progress,
  type RepoRecord,
  type RepoView,
  type SearchKind,
  type Submodule,
  type Settings,
  type SyncAction,
  type UpdateInfo,
} from "./api";
import { accentTokens, monoStack } from "./lib/appearance";
import { EMPTY_DRAFT, persistDraft, savedDrafts, type CommitDraft } from "./lib/drafts";
import { ask } from "./components/Prompt";
import { applySidebarWidth, lastSidebarWidth, repoLayout } from "./lib/layout";
import { claimMissingLfsWarning, LFS_INSTALL_URL } from "./lib/lfs";
import { resolveBindings } from "./lib/keymap";
import { reconcile, type FileSel } from "./lib/selection";
import { closeTab, cycleTab, placeTab } from "./lib/tabs";

export type Tab = "changes" | "history";
export type Popover = "repos" | "branches" | null;

export type Dialog =
  | { type: "addRepo"; initPath?: string }
  | { type: "createRepo" }
  | { type: "newBranch"; start?: string }
  | { type: "renameBranch"; name: string }
  | { type: "setUpstream"; branch: string; current: string | null }
  | { type: "settings"; tab?: string }
  | { type: "signIn" }
  | { type: "merge" }
  | { type: "about" }
  | { type: "shortcuts" }
  | { type: "stashes" }
  | { type: "clone"; url?: string }
  | { type: "publish" }
  | { type: "repoSettings" }
  | { type: "import" }
  | { type: "fetchAll" }
  | { type: "rebase" }
  | { type: "tag"; sha: string }
  | { type: "reset"; sha: string }
  | { type: "cherryPick"; shas: string[] }
  | { type: "interactiveRebase"; sha: string }
  | { type: "squash"; target: string; commits: string[] }
  | { type: "undoHistory" }
  | { type: "worktrees" }
  | { type: "applyPatch" }
  | { type: "blame"; path: string }
  | { type: "fileHistory"; path: string }
  | { type: "mergeEditor"; path: string }
  | { type: "lfsTrack"; path: string }
  | { type: "checks" }
  | { type: "checkLog"; jobId: number; name: string }
  | { type: "previewPullRequest" }
  | { type: "review"; number: number; title: string }
  | { type: "whatsNew"; notes: ReleaseNote[] };

export interface Banner {
  id: number;
  tone: "info" | "success" | "error";
  message: string;
  detail?: string | null;
  action?: { label: string; run: () => void };
  onDismiss?: () => void;
  sticky?: boolean;
  duration?: number;
  leaving?: boolean;
}

interface AppStore {
  ready: boolean;
  repositories: RepoRecord[];
  settings: Settings | null;
  account: Account | null;
  oauthConfigured: boolean;
  gitFound: boolean;
  repo: RepoView | null;
  tab: Tab;
  selectedPath: string | null;
  checkedPaths: Record<string, FileSel>;
  popover: Popover;
  dialog: Dialog | null;
  banners: Banner[];
  sync: { action: SyncAction; progress: Progress | null; label?: string } | null;
  busy: string | null;
  opening: string | null;
  checks: ChecksSummary | null;
  submodules: Submodule[];
  lfs: (LfsStatus & { locks: LfsLock[] | null }) | null;
  commitWarnings: CommitWarnings | null;
  historySearch: { kind: SearchKind; query: string } | null;
  compareWith: string | null;
  tabs: string[];
  pickingNewTab: boolean;
  drafts: Record<string, CommitDraft>;
}

export const [store, setStore] = createStore<AppStore>({
  ready: false,
  repositories: [],
  settings: null,
  account: null,
  oauthConfigured: false,
  gitFound: true,
  repo: null,
  tab: "changes",
  selectedPath: null,
  checkedPaths: {},
  popover: null,
  dialog: null,
  banners: [],
  sync: null,
  busy: null,
  opening: null,
  checks: null,
  submodules: [],
  lfs: null,
  commitWarnings: null,
  historySearch: null,
  compareWith: null,
  tabs: [],
  pickingNewTab: false,
  drafts: savedDrafts(),
});

export function commitDraft(repo: string): CommitDraft {
  return store.drafts[repo] ?? EMPTY_DRAFT;
}

export function updateDraft(repo: string, patch: Partial<CommitDraft>) {
  const next = { ...commitDraft(repo), ...patch };
  setStore("drafts", repo, next);
  persistDraft(repo, next);
}

const diffCache = new Map<string, FileDiff>();
export const [diffVersion, setDiffVersion] = createSignal(0);

export function cachedDiff(path: string): FileDiff | undefined {
  diffVersion();
  return diffCache.get(path);
}

export function sameDiff(a: FileDiff | undefined, b: FileDiff): boolean {
  if (!a || a.additions !== b.additions || a.deletions !== b.deletions || a.hunks.length !== b.hunks.length) return false;
  return JSON.stringify(a) === JSON.stringify(b);
}

export function storeDiff(path: string, diff: FileDiff) {
  if (sameDiff(diffCache.get(path), diff)) return;
  diffCache.set(path, diff);
  const existing = store.checkedPaths[path];
  const next = reconcile(existing, diff);
  if (next !== existing && next) setStore("checkedPaths", path, next);
  setDiffVersion((v) => v + 1);
}

export const TOAST_EXIT_MS = 180;
let bannerSeq = 0;
export function notify(tone: Banner["tone"], message: string, extra: Partial<Banner> = {}) {
  const id = ++bannerSeq;
  const duration = tone === "error" || extra.sticky ? undefined : extra.action ? 8000 : 4000;
  setStore("banners", (b) => [...b.slice(-5), { id, tone, message, duration, ...extra }]);
}

export function dismiss(id: number) {
  if (!store.banners.some((b) => b.id === id && !b.leaving)) return;
  setStore("banners", (b) => b.id === id, "leaving", true);
  setTimeout(() => setStore("banners", (b) => b.filter((x) => x.id !== id)), TOAST_EXIT_MS);
}

function authorizeAction(url: string | null): Banner["action"] {
  if (!url?.startsWith("https://")) return undefined;
  return { label: "Authorize", run: () => api.openExternal(url).catch(reportError) };
}

function removeLockAction(path: string) {
  return {
    label: "Remove lock",
    run: () => {
      const repo = store.repo;
      if (!repo) return;
      api
        .removeStaleLock(repo.id, path)
        .then(() => notify("success", "Lock removed. Try again."))
        .catch((err) => reportError(err));
    },
  };
}

export function reportError(e: unknown, context?: string) {
  if (isCommandError(e)) {
    if (e.kind === "cancelled") return;
    if (e.kind === "sso") {
      notify("error", "This organization requires single sign-on", { sticky: true, action: authorizeAction(e.detail) });
      return;
    }
    if (e.kind === "locked" && e.detail?.endsWith(".lock")) {
      notify("error", `${context ? `${context}: ` : ""}${e.message}. If no other Git tool is running, the lock is left over from a crash.`, {
        detail: e.detail,
        action: removeLockAction(e.detail),
      });
      return;
    }
    notify("error", context ? `${context}: ${e.message}` : e.message, { detail: e.detail });
  } else {
    notify("error", context ? `${context}: ${String(e)}` : String(e));
  }
}

export async function run<T>(label: string, task: () => Promise<T>): Promise<T | undefined> {
  setStore("busy", label);
  try {
    return await task();
  } catch (e) {
    reportError(e, label);
    return undefined;
  } finally {
    setStore("busy", null);
  }
}

const tabViews = new Map<string, RepoView>();
const TABS_KEY = "blink.tabs";

function saveTabs() {
  if (windowRepo) return;
  try {
    localStorage.setItem(TABS_KEY, JSON.stringify(store.tabs));
  } catch {}
}

function savedTabs(): string[] {
  try {
    const parsed = JSON.parse(localStorage.getItem(TABS_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((t): t is string => typeof t === "string") : [];
  } catch {
    return [];
  }
}

function setTabs(tabs: string[]) {
  setStore("tabs", tabs);
  for (const path of tabViews.keys()) if (!tabs.includes(path)) tabViews.delete(path);
  saveTabs();
}

export function openInNewTab(path: string) {
  return openRepo(path, true);
}

export function pickNewTab() {
  setStore({ pickingNewTab: true, popover: "repos" });
}

export function closeRepoTab(path: string) {
  const { tabs, next } = closeTab(store.tabs, path, store.repo?.id ?? null);
  setTabs(tabs);
  if (next === null) setStore("repo", null);
  else if (next !== store.repo?.id) void openRepo(next);
}

export function cycleRepoTab(step: number) {
  const next = cycleTab(store.tabs, store.repo?.id ?? null, step);
  if (next && next !== store.repo?.id) void openRepo(next);
}

export function applyRepoView(view: RepoView) {
  if (store.tabs.includes(view.id)) tabViews.set(view.id, view);
  if (store.repo && store.repo.id !== view.id) return;
  if (store.repo && view.generation < store.repo.generation) return;
  batch(() => {
    setStore("repo", reconcileStore(view, { key: "path" }));
    const present = new Set(view.files.map((f) => f.path));
    setStore(
      "checkedPaths",
      produce((sel) => {
        for (const path of Object.keys(sel)) if (!present.has(path)) delete sel[path];
      }),
    );
    if (store.selectedPath && !present.has(store.selectedPath)) {
      setStore("selectedPath", view.files[0]?.path ?? null);
    }
    if (!store.selectedPath && view.files.length) setStore("selectedPath", view.files[0].path);
    for (const path of [...diffCache.keys()]) if (!present.has(path)) diffCache.delete(path);
  });
}

export const [keymap, setKeymap] = createSignal(resolveBindings({}));

export const shortcut = (id: string): string | undefined => keymap().keys[id] || undefined;

export async function loadKeymap() {
  try {
    setKeymap(resolveBindings((await api.keymap()) ?? {}));
  } catch (e) {
    reportError(e, "Could not read keymap.toml");
  }
}

export const windowRepo = new URLSearchParams(location.search).get("repo");

export async function bootstrap() {
  const boot: Bootstrap = await api.bootstrap();
  setStore({
    repositories: boot.repositories,
    settings: boot.settings,
    account: boot.account,
    oauthConfigured: boot.oauthConfigured,
    gitFound: boot.gitFound,
    ready: true,
  });
  applyTheme();
  setTimeout(() => document.documentElement.classList.remove("preload"), 60);
  api
    .takeCrashReport()
    .then(
      (report) =>
        report &&
        notify("error", "Blink closed unexpectedly last time. A crash report was saved to the logs folder.", {
          action: { label: "Open logs folder", run: () => api.openLogsFolder().catch(reportError) },
        }),
    )
    .catch(() => {});
  const known = new Set(boot.repositories.map((r) => r.path));
  if (!windowRepo) setStore("tabs", savedTabs().filter((t) => known.has(t)));
  const last = windowRepo ?? boot.lastRepository ?? boot.repositories[0]?.path;
  if (last && known.has(last)) await openRepo(last);
}

export function openInNewWindow(path: string) {
  setStore("popover", null);
  api.openWindow(path).catch((e) => reportError(e, "Could not open a new window"));
}

function showRepo(view: RepoView) {
  const memory = repoLayout(view.id);
  const selected = memory.selectedPath;
  applySidebarWidth(memory.sidebar ?? lastSidebarWidth());
  diffCache.clear();
  batch(() => {
    setStore({
      repo: view,
      tab: memory.tab ?? store.tab,
      checkedPaths: {},
      selectedPath: selected && view.files.some((f) => f.path === selected) ? selected : (view.files[0]?.path ?? null),
      checks: null,
      submodules: [],
      lfs: null,
      historySearch: null,
      compareWith: null,
    });
  });
  setDiffVersion((v) => v + 1);
}

export async function openRepo(path: string, asNewTab = store.pickingNewTab) {
  const current = store.repo?.id ?? null;
  const previousTabs = store.tabs;
  setTabs(placeTab(store.tabs, current, path, asNewTab));
  setStore({ popover: null, opening: path, pickingNewTab: false });
  const request = api.openRepository(path);
  let cached = tabViews.get(path);
  if (!cached && current !== path) {
    const remembered = await api.cachedRepository(path).catch(() => null);
    // generations restart every session, so a remembered view must never outrank a fresh one
    if (remembered && store.opening === path) cached = { ...remembered, generation: -1 };
  }
  if (cached && current !== path) showRepo(cached);
  try {
    const view = await request;
    tabViews.set(path, view);
    if (cached && store.repo?.id === path) applyRepoView(view);
    else showRepo(view);
    void offerFsmonitor(view.id);
  } catch (e) {
    if (store.repo?.id === path && cached) setStore("repo", null);
    if (store.repo?.id !== path) setTabs(previousTabs);
    reportError(e, "Could not open repository");
  } finally {
    if (store.opening === path) setStore("opening", null);
  }
}

const fsmonitorOffered = new Set<string>();

async function offerFsmonitor(id: string) {
  if (fsmonitorOffered.has(id)) return;
  fsmonitorOffered.add(id);
  if (!(await api.fsmonitorSuggestion(id).catch(() => false))) return;
  notify("info", "This repository is large. Turn on Git's file system monitor to speed up status?", {
    sticky: true,
    action: { label: "Turn on", run: () => void enableFsmonitor(id) },
    onDismiss: () => void api.dismissFsmonitorSuggestion(id).catch(() => {}),
  });
}

async function enableFsmonitor(id: string) {
  try {
    await api.enableFsmonitor(id);
    notify("success", "Git's file system monitor is on for this repository.");
  } catch (e) {
    reportError(e, "Could not turn on the file system monitor");
  }
}

export async function refreshRepo() {
  const repo = store.repo;
  if (!repo) return;
  try {
    applyRepoView(await api.refreshRepository(repo.id));
  } catch (e) {
    reportError(e);
  }
}

export async function reloadRepositories() {
  const shared = await api.sharedState();
  setStore("repositories", shared.repositories);
}

export async function reloadSharedState() {
  const shared = await api.sharedState();
  setStore({ repositories: shared.repositories, settings: shared.settings, account: shared.account });
  applyTheme();
}

export async function addRepository(path: string) {
  try {
    const record = await api.addRepository(path);
    await reloadRepositories();
    await openRepo(record.path);
    setStore("dialog", null);
  } catch (e) {
    if (isCommandError(e) && e.kind === "notARepository") {
      setStore("dialog", { type: "addRepo", initPath: path });
    } else {
      reportError(e, "Could not add repository");
    }
  }
}

const ACCENT_PROPERTIES = ["--accent", "--accent-fg", "--bg-selected-focus", "--fg-selected-focus", "--bg-selected"];

export function applyTheme(settings: Settings | null = store.settings) {
  const root = document.documentElement;
  const theme = settings?.appearance.theme ?? "system";
  const dark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  root.dataset.theme = dark ? "dark" : "light";
  root.dataset.density = settings?.appearance.density ?? "comfortable";
  const a11y = settings?.accessibility;
  root.classList.toggle("reduced-motion", !!a11y?.reducedMotion);
  root.classList.toggle("high-contrast", !!a11y?.highContrast);
  root.classList.toggle("underline-links", !!a11y?.underlineLinks);
  root.classList.toggle("large-hit-targets", !!a11y?.largeHitTargets);
  root.classList.toggle("keyboard-navigation", !!a11y?.keyboardNavigation);
  const accent = accentTokens(settings?.appearance.accentColor);
  for (const prop of ACCENT_PROPERTIES) {
    if (accent?.[prop]) root.style.setProperty(prop, accent[prop]);
    else root.style.removeProperty(prop);
  }
  const mono = monoStack(settings?.appearance.diffFont);
  if (mono) root.style.setProperty("--font-mono", mono);
  else root.style.removeProperty("--font-mono");
}

export async function saveSettings(settings: Settings) {
  await api.saveSettings(settings);
  setStore("settings", settings);
  applyTheme();
}

export function currentBranch(): string | null {
  const head = store.repo?.head;
  if (!head) return null;
  return head.type === "detached" ? null : head.name;
}

export async function refreshCommitWarnings() {
  const repo = store.repo;
  if (!repo) return setStore("commitWarnings", null);
  try {
    const warnings = await api.commitWarnings(repo.id);
    if (store.repo?.id === repo.id) setStore("commitWarnings", warnings);
  } catch {
    setStore("commitWarnings", null);
  }
}

export async function refreshChecks() {
  const repo = store.repo;
  if (!repo) return setStore("checks", null);
  try {
    const checks = await api.branchChecks(repo.id);
    if (store.repo?.id === repo.id) setStore("checks", checks);
  } catch {
    setStore("checks", null);
  }
}

export async function refreshSubmodules() {
  const repo = store.repo;
  if (!repo) return setStore("submodules", []);
  const list = await api.submodules(repo.id).catch(() => []);
  if (store.repo?.id === repo.id) setStore("submodules", list);
}

export async function updateSubmodules(paths?: string[]) {
  const repo = store.repo;
  if (!repo || store.sync) return;
  setStore("sync", { action: "fetch", progress: null, label: "Updating submodules" });
  try {
    await api.submoduleUpdate(repo.id, paths);
    notify("success", paths?.length === 1 ? `Updated ${paths[0]}` : "Submodules are up to date");
  } catch (e) {
    reportError(e, "Could not update submodules");
  } finally {
    setStore("sync", null);
    await refreshSubmodules();
  }
}

export async function initSubmodules() {
  const repo = store.repo;
  if (!repo) return;
  const done = await run("Initialize submodules", () => api.submoduleInit(repo.id).then(() => true));
  if (done) notify("success", "Submodules initialized. Update them to check out their files.", { action: { label: "Update", run: () => void updateSubmodules() } });
  await refreshSubmodules();
}

export async function syncSubmodules() {
  const repo = store.repo;
  if (!repo) return;
  const done = await run("Sync submodules", () => api.submoduleSync(repo.id).then(() => true));
  if (done) notify("success", "Submodule URLs synced from .gitmodules");
}

export async function openSubmodule(path: string) {
  const repo = store.repo;
  if (!repo) return;
  try {
    const record = await api.openSubmodule(repo.id, path);
    await reloadRepositories();
    await openRepo(record.path, true);
  } catch (e) {
    reportError(e, `Could not open ${path}`);
  }
}

function warnMissingLfs(id: string) {
  let storage: Storage;
  try {
    storage = localStorage;
  } catch {
    return;
  }
  if (!claimMissingLfsWarning(id, storage)) return;
  notify("info", "This repository stores files with Git LFS, but Git LFS isn't installed. Those files show up as small pointer files until you install it.", {
    sticky: true,
    action: { label: "Install Git LFS", run: () => api.openExternal(LFS_INSTALL_URL).catch(reportError) },
  });
}

export async function refreshLfs() {
  const repo = store.repo;
  if (!repo) return setStore("lfs", null);
  const status = await api.lfsStatus(repo.id).catch(() => null);
  if (store.repo?.id !== repo.id) return;
  setStore("lfs", status ? { ...status, locks: null } : null);
  if (!status?.usesLfs) return;
  if (!status.version) warnMissingLfs(repo.id);
  else if (repo.hasRemote) await refreshLfsLocks();
}

export async function refreshLfsLocks() {
  const repo = store.repo;
  if (!repo || !store.lfs?.version) return;
  const locks = await api.lfsLocks(repo.id).catch(() => null);
  if (store.repo?.id === repo.id && store.lfs) setStore("lfs", "locks", locks);
}

export function lfsLockFor(path: string): LfsLock | undefined {
  return store.lfs?.locks?.find((l) => l.path === path);
}

export async function lockFile(path: string) {
  const repo = store.repo;
  if (!repo) return;
  const done = await run("Lock file", () => api.lfsLock(repo.id, path).then(() => true));
  if (done) notify("success", `Locked ${path}`);
  await refreshLfsLocks();
}

export async function unlockFile(path: string, force = false) {
  const repo = store.repo;
  if (!repo) return;
  if (force) {
    const owner = lfsLockFor(path)?.owner ?? "someone else";
    const ok = await ask(`${path} is locked by ${owner}. Force the unlock? They may lose work if they're still editing it.`, {
      title: "Force unlock",
      kind: "warning",
      okLabel: "Force unlock",
    });
    if (!ok) return;
  }
  const done = await run("Unlock file", () => api.lfsUnlock(repo.id, path, force).then(() => true));
  if (done) notify("success", `Unlocked ${path}`);
  await refreshLfsLocks();
}

export async function pullLfs() {
  const repo = store.repo;
  if (!repo || store.sync) return;
  setStore("sync", { action: "fetch", progress: null, label: "Pulling LFS files" });
  try {
    await api.lfsPull(repo.id);
    notify("success", "Git LFS files are up to date");
  } catch (e) {
    reportError(e, "Could not pull LFS files");
  } finally {
    setStore("sync", null);
  }
}

export async function undoLastOperation() {
  const repo = store.repo;
  if (!repo) return;
  try {
    const entry = await api.undo(repo.id);
    notify("success", `Undid "${entry.label}"`);
  } catch (e) {
    reportError(e, "Undo failed");
  }
}

export async function confirmUndoCommit(what: string): Promise<boolean> {
  if (store.settings?.prompts.undoCommit === false) return true;
  return ask(`Undo ${what}? Its changes go back to the Changes list.`, { title: "Undo commit", okLabel: "Undo" });
}

export async function undoHeadCommit(summary: string) {
  const repo = store.repo;
  if (!repo || !(await confirmUndoCommit(`"${summary}"`))) return;
  await run("Undo commit", () => api.undoLastCommit(repo.id));
}

let updateOffered: string | null = null;

export function offerUpdate(info: UpdateInfo) {
  if (updateOffered === info.version) return;
  updateOffered = info.version;
  notify("info", `Blink ${info.version} is ready to install.`, {
    sticky: true,
    action: { label: "Restart to update", run: installUpdate },
  });
}

export async function installUpdate() {
  setStore("busy", "Downloading update");
  try {
    await api.installUpdate();
  } catch (e) {
    reportError(e, "Update failed");
  } finally {
    setStore("busy", null);
  }
}

export async function checkForUpdates(manual = false) {
  try {
    const info = await api.checkForUpdate();
    if (info) {
      updateOffered = null;
      offerUpdate(info);
    } else if (manual) {
      notify("success", "You're on the latest version of Blink.");
    }
  } catch (e) {
    if (manual) reportError(e, "Could not check for updates");
  }
}

export async function showWhatsNew(all = false) {
  try {
    const notes = all ? await api.releaseNotes() : await api.whatsNew();
    if (notes.length) setStore("dialog", { type: "whatsNew", notes });
    else if (all) notify("info", "No release notes yet.");
  } catch (e) {
    if (all) reportError(e, "Could not load release notes");
  }
}

export async function handleLaunches() {
  const launches = await api.takePendingLaunch().catch(() => []);
  for (const launch of launches) {
    if (launch.type === "open") {
      const trusted =
        !launch.fromLink ||
        (await ask(`A link wants to add ${launch.path} to Blink. Only continue if you trust this folder.`, {
          title: "Add repository",
          kind: "warning",
          okLabel: "Add",
        }));
      if (trusted) await addRepository(launch.path);
    } else setStore("dialog", { type: "clone", url: launch.url });
  }
}
