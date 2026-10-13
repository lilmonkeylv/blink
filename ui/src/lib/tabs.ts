export function placeTab(tabs: string[], active: string | null, path: string, asNew: boolean): string[] {
  if (tabs.includes(path)) return tabs;
  const at = active ? tabs.indexOf(active) : -1;
  if (asNew || at < 0) return at < 0 ? [...tabs, path] : [...tabs.slice(0, at + 1), path, ...tabs.slice(at + 1)];
  return tabs.map((t, i) => (i === at ? path : t));
}

export function closeTab(tabs: string[], path: string, active: string | null): { tabs: string[]; next: string | null } {
  const at = tabs.indexOf(path);
  if (at < 0) return { tabs, next: active };
  const rest = tabs.filter((t) => t !== path);
  if (path !== active) return { tabs: rest, next: active };
  return { tabs: rest, next: rest[Math.min(at, rest.length - 1)] ?? null };
}

export function cycleTab(tabs: string[], active: string | null, step: number): string | null {
  if (!tabs.length) return null;
  const at = active ? tabs.indexOf(active) : -1;
  return tabs[(((at + step) % tabs.length) + tabs.length) % tabs.length];
}
