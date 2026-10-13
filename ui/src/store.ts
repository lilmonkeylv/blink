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
  type Progress,
  type RepoRecord,
  type RepoView,
  type SearchKind,
  type Settings,
  type SyncAction,
  type UpdateInfo,
} from "./api";
import { applySidebarWidth, lastSidebarWidth, repoLayout } from "./lib/layout";
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
  | { type: "checks" }
  | { type: "checkLog"; jobId: number; name: string }
  | { type: "previewPullRequest" }
  | { type: "whatsNew"; notes: ReleaseNote[] };

export interface Banner {
  id: number;
  tone: "info" | "success" | "error";
  message: string;
  detail?: string | null;
  action?: { label: string; run: () => void };
  onDismiss?: () => void;
  sticky?: boolean;
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
  sync: { action: SyncAction; progress: Progress | null } | null;
  busy: string | null;
  opening: string | null;
  checks: ChecksSummary | null;
  commitWarnings: CommitWarnings | null;
  historySearch: { kind: SearchKind; query: string } | null;
  compareWith: string | null;
  tabs: string[];
  pickingNewTab: boolean;
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
  commitWarnings: null,
  historySearch: null,
  compareWith: null,
  tabs: [],
  pickingNewTab: false,
});

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

let bannerSeq = 0;
export function notify(tone: Banner["tone"], message: string, extra: Partial<Banner> = {}) {
  const id = ++bannerSeq;
  setStore("banners", (b) => [...b.slice(-3), { id, tone, message, ...extra }]);
  if (tone !== "error" && !extra.sticky) setTimeout(() => dismiss(id), extra.action ? 8000 : 4000);
}

export function dismiss(id: number) {
  setStore("banners", (b) => b.filter((x) => x.id !== id));
}

function authorizeAction(url: string | null): Banner["action"] {
  if (!url?.startsWith("https://")) return undefined;
  return { label: "Authorize", run: () => api.openExternal(url).catch(reportError) };
}

export function reportError(e: unknown, context?: string) {
  if (isCommandError(e)) {
    if (e.kind === "cancelled") return;
    if (e.kind === "sso") {
      notify("error", "This organization requires single sign-on", { sticky: true, action: authorizeAction(e.detail) });
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
  let cached = tabViews.get(path);
  if (!cached && current !== path) {
    const remembered = await api.cachedRepository(path).catch(() => null);
    // generations restart every session, so a remembered view must never outrank a fresh one
    if (remembered && store.opening === path) cached = { ...remembered, generation: -1 };
  }
  if (cached && current !== path) showRepo(cached);
  try {
    const view = await api.openRepository(path);
    tabViews.set(path, view);
    if (cached && store.repo?.id === path) applyRepoView(view);
    else showRepo(view);
    void offerFsmonitor(view.id);
  } catch (e) {
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

export function applyTheme() {
  const theme = store.settings?.appearance.theme ?? "system";
  const dark = theme === "dark" || (theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  const density = store.settings?.appearance.density ?? "comfortable";
  document.documentElement.dataset.density = density;
  const a11y = store.settings?.accessibility;
  document.documentElement.classList.toggle("reduced-motion", !!a11y?.reducedMotion);
  document.documentElement.classList.toggle("high-contrast", !!a11y?.highContrast);
  document.documentElement.classList.toggle("underline-links", !!a11y?.underlineLinks);
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
    if (launch.type === "open") await addRepository(launch.path);
    else setStore("dialog", { type: "clone", url: launch.url });
  }
}
