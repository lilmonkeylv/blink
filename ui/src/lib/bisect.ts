import type { BisectState } from "../api";

export type BisectPhase = "waiting" | "testing" | "found";
export type BisectRole = "bad" | "good" | "skipped" | "current" | null;

const short = (sha: string | null | undefined) => (sha ?? "").slice(0, 7);
const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

export function bisectPhase(state: BisectState): BisectPhase {
  if (state.firstBad) return "found";
  return state.bad && state.good.length ? "testing" : "waiting";
}

export function bisectStatus(state: BisectState): string {
  switch (bisectPhase(state)) {
    case "found":
      return `${short(state.firstBad)} is the first bad commit${state.firstBadSummary ? `: ${state.firstBadSummary}` : ""}`;
    case "testing": {
      const subject = state.currentSummary ? ` ${state.currentSummary}` : "";
      const steps = state.stepsLeft === null ? "" : `, about ${plural(state.stepsLeft, "step")} left`;
      return `Bisecting: testing ${short(state.current)}${subject}${steps}`;
    }
    default:
      return state.bad
        ? `Bisecting: ${short(state.bad)} is bad. Mark a commit you know is good in History.`
        : `Bisecting: mark a commit you know is bad in History.`;
  }
}

export function bisectRole(state: BisectState | null, sha: string): BisectRole {
  if (!state) return null;
  if (state.bad === sha) return "bad";
  if (state.good.includes(sha)) return "good";
  if (state.skipped.includes(sha)) return "skipped";
  return state.current === sha && !state.firstBad ? "current" : null;
}
