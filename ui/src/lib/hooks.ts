import type { HookAction, HookEvent, HookResult } from "../api";

export const MAX_HOOK_LINES = 400;

export interface HookRun {
  repo: string;
  action: HookAction;
  hooks: string[];
  lines: string[];
  dropped: number;
  results: HookResult[];
  status: "running" | "passed" | "failed";
}

export function applyHookEvent(run: HookRun | null, repo: string, action: HookAction, event: HookEvent): HookRun | null {
  if (event.type === "started") {
    return { repo, action, hooks: event.hooks, lines: [], dropped: 0, results: [], status: "running" };
  }
  const current = run && run.repo === repo && run.action === action ? run : null;
  if (event.type === "output") {
    if (!current || current.status !== "running") return run;
    const lines = [...current.lines, event.line];
    const overflow = Math.max(0, lines.length - MAX_HOOK_LINES);
    return { ...current, lines: overflow ? lines.slice(overflow) : lines, dropped: current.dropped + overflow };
  }
  const base = current ?? { repo, action, hooks: event.results.map((r) => r.name), lines: [], dropped: 0, results: [], status: "running" as const };
  return { ...base, results: event.results, status: event.results.every((r) => r.code === 0) ? "passed" : "failed" };
}

export function failedHook(run: HookRun): HookResult | null {
  return run.results.find((r) => r.code !== 0) ?? null;
}

export function hookTitle(run: HookRun): string {
  const names = run.hooks.length ? run.hooks.join(", ") : `${run.action} hooks`;
  if (run.status === "running") return `Running ${names}`;
  const failed = failedHook(run);
  if (failed) return `${failed.name} hook failed${failed.code === null ? "" : ` (exit ${failed.code})`}`;
  const count = run.results.length;
  return count === 1 ? `${run.results[0].name} hook passed` : `${count} hooks passed`;
}
