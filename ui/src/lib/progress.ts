import type { Progress } from "../api";

const TRANSFER_PHASES = /^(receiving objects|resolving deltas|writing objects|updating files|checking out files)/i;

export function transferPercent(progress: Progress | null | undefined): number | null {
  if (!progress || progress.percent === null || progress.percent >= 100) return null;
  return TRANSFER_PHASES.test(progress.phase) ? progress.percent : null;
}
