import { batch, createSignal } from "solid-js";
import { createStore, produce, reconcile as reconcileStore } from "solid-js/store";
import {
  api,
  isCommandError,
  type Account,
  type Bootstrap,
  type ChecksSummary,
  type FileDiff,
  type Progress,
  type RepoRecord,
  type RepoView,
  type SearchKind,
  type Settings,
  type SyncAction,
} from "./api";
import { reconcile, type FileSel } from "./lib/selection";

export type Tab = "changes" | "history";
export type Popover = "repos" | "branches" | null;

export type Dialog =
  | { type: "addRepo"; initPath?: string }
  | { type: "createRepo" }
  | { type: "newBranch"; start?: string }
  | { type: "renameBranch"; name: string }
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
  | { type: "checks" };

export interface Banner {
  id: number;
  tone: "info" | "success" | "error";
  message: string;
  detail?: string | null;
  action?: { label: string; run: () => void };
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
  checks: ChecksSummary | null;
  historySearch: { kind: SearchKind; query: string } | null;
  compareWith: string | null;
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
  checks: null,
  historySearch: null,
  compareWith: null,
});

const diffCache = new Map<string, FileDiff>();
export const [diffVersion, setDiffVersion] = createSignal(0);

export function cachedDiff(path: string): FileDiff | undefined {
  diffVersion();
  return diffCache.get(path);
}

export function storeDiff(path: string, diff: FileDiff) {
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
  if (tone !== "error") setTimeout(() => dismiss(id), extra.action ? 8000 : 4000);
}

export function dismiss(id: number) {
  setStore("banners", (b) => b.filter((x) => x.id !== id));
}

export function reportError(e: unknown, context?: string) {
  if (isCommandError(e)) {
    if (e.kind === "cancelled") return;
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

export function applyRepoView(view: RepoView) {
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
  });
  diffCache.clear();
  setDiffVersion((v) => v + 1);
}

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
  const last = boot.lastRepository ?? boot.repositories[0]?.path;
  if (last && boot.repositories.some((r) => r.path === last)) await openRepo(last);
}

export async function openRepo(path: string) {
  setStore("popover", null);
  try {
    const view = await api.openRepository(path);
    diffCache.clear();
    batch(() => {
      setStore({
        repo: view,
        checkedPaths: {},
        selectedPath: view.files[0]?.path ?? null,
        checks: null,
        historySearch: null,
        compareWith: null,
      });
    });
    setDiffVersion((v) => v + 1);
  } catch (e) {
    reportError(e, "Could not open repository");
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
  const boot = await api.bootstrap();
  setStore("repositories", boot.repositories);
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

export async function refreshChecks() {
  const repo = store.repo;
  if (!repo || !store.account) return setStore("checks", null);
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

export async function handleLaunches() {
  const launches = await api.takePendingLaunch().catch(() => []);
  for (const launch of launches) {
    if (launch.type === "open") await addRepository(launch.path);
    else setStore("dialog", { type: "clone", url: launch.url });
  }
}
