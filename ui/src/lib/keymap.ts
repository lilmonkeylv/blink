export interface Binding {
  id: string;
  keys: string;
  label: string;
}

export const DEFAULT_BINDINGS: Binding[] = [
  { id: "view.changes", keys: "Ctrl+1", label: "Changes" },
  { id: "view.history", keys: "Ctrl+2", label: "History" },
  { id: "repo.list", keys: "Ctrl+T", label: "Repository list" },
  { id: "tab.new", keys: "Ctrl+Alt+T", label: "Open a repository in a new tab" },
  { id: "tab.next", keys: "Ctrl+Tab", label: "Next tab" },
  { id: "tab.previous", keys: "Ctrl+Shift+Tab", label: "Previous tab" },
  { id: "tab.nextPage", keys: "Ctrl+PageDown", label: "Next tab" },
  { id: "tab.previousPage", keys: "Ctrl+PageUp", label: "Previous tab" },
  { id: "tab.close", keys: "Ctrl+W", label: "Close tab" },
  { id: "branch.list", keys: "Ctrl+B", label: "Branch list" },
  { id: "branch.new", keys: "Ctrl+Shift+N", label: "New branch" },
  { id: "repo.add", keys: "Ctrl+O", label: "Add local repository" },
  { id: "repo.clone", keys: "Ctrl+Shift+O", label: "Clone repository" },
  { id: "repo.create", keys: "Ctrl+N", label: "New repository" },
  { id: "sync.push", keys: "Ctrl+P", label: "Push" },
  { id: "sync.pull", keys: "Ctrl+Shift+P", label: "Pull" },
  { id: "sync.fetch", keys: "Ctrl+Shift+T", label: "Fetch" },
  { id: "undo", keys: "Ctrl+Z", label: "Undo last Git operation" },
  { id: "history.search", keys: "Ctrl+F", label: "Search history" },
  { id: "pr.create", keys: "Ctrl+R", label: "Create pull request" },
  { id: "open.editor", keys: "Ctrl+Shift+A", label: "Open in external editor" },
  { id: "open.shell", keys: "Ctrl+`", label: "Open in shell" },
  { id: "open.files", keys: "Ctrl+Shift+F", label: "Show in file manager" },
  { id: "open.github", keys: "Ctrl+Shift+G", label: "View on GitHub" },
  { id: "file.blame", keys: "Ctrl+Shift+L", label: "Blame selected file" },
  { id: "stash.all", keys: "Ctrl+Shift+S", label: "Stash all changes" },
  { id: "palette", keys: "Ctrl+K", label: "Command palette" },
  { id: "settings", keys: "Ctrl+,", label: "Settings" },
  { id: "refresh", keys: "F5", label: "Refresh" },
];

const MODIFIERS = ["ctrl", "alt", "shift"] as const;

export function normalizeCombo(combo: string): string | null {
  const parts = combo
    .split("+")
    .map((p) => p.trim().toLowerCase())
    .filter(Boolean)
    .map((p) => (p === "cmd" || p === "meta" || p === "control" ? "ctrl" : p === "option" ? "alt" : p));
  const key = parts.filter((p) => !(MODIFIERS as readonly string[]).includes(p));
  if (key.length !== 1) return null;
  return [...MODIFIERS.filter((m) => parts.includes(m)), key[0]].join("+");
}

export function comboFromEvent(e: Pick<KeyboardEvent, "key" | "ctrlKey" | "metaKey" | "altKey" | "shiftKey">): string {
  const key = e.key === " " ? "space" : e.key.toLowerCase();
  const mods = [e.ctrlKey || e.metaKey ? "ctrl" : null, e.altKey ? "alt" : null, e.shiftKey ? "shift" : null];
  return [...mods.filter(Boolean), key].join("+");
}

export function resolveBindings(overrides: Record<string, string>): { byCombo: Map<string, string>; keys: Record<string, string> } {
  const keys: Record<string, string> = {};
  for (const b of DEFAULT_BINDINGS) keys[b.id] = b.id in overrides ? overrides[b.id] : b.keys;
  const byCombo = new Map<string, string>();
  for (const [id, combo] of Object.entries(keys)) {
    const normalized = combo ? normalizeCombo(combo) : null;
    if (normalized && !byCombo.has(normalized)) byCombo.set(normalized, id);
  }
  return { byCombo, keys };
}
