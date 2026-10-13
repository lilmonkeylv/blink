export interface RepoLayout {
  tab?: "changes" | "history";
  selectedPath?: string | null;
  commit?: string | null;
  sidebar?: number;
}

const KEY = "blink.layout";
const MAX_REPOS = 200;
export const SIDEBAR_DEFAULT = 320;

export function clampSidebar(px: number, viewport = window.innerWidth): number {
  return Math.round(Math.max(220, Math.min(px, 640, viewport - 360)));
}

function readAll(): Record<string, RepoLayout> {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

export function mergeLayout(all: Record<string, RepoLayout>, repo: string, patch: RepoLayout, max = MAX_REPOS) {
  const { [repo]: previous, ...rest } = all;
  const entries = Object.entries(rest).slice(-(max - 1));
  return { ...Object.fromEntries(entries), [repo]: { ...previous, ...patch } };
}

export function repoLayout(repo: string): RepoLayout {
  return readAll()[repo] ?? {};
}

export function saveRepoLayout(repo: string, patch: RepoLayout) {
  try {
    localStorage.setItem(KEY, JSON.stringify(mergeLayout(readAll(), repo, patch)));
  } catch {}
}

export function lastSidebarWidth(): number {
  const widths = Object.values(readAll())
    .map((l) => l.sidebar)
    .filter((w): w is number => typeof w === "number");
  return widths[widths.length - 1] ?? SIDEBAR_DEFAULT;
}

export function applySidebarWidth(px: number) {
  document.documentElement.style.setProperty("--sidebar", `${clampSidebar(px)}px`);
}
