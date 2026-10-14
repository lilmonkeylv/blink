import type { MergePrediction } from "../api";

export type PredictionTone = "ok" | "warn" | "info";

export interface PredictionSummary {
  tone: PredictionTone;
  text: string;
  files: string[];
}

const plural = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

export function describePrediction(
  prediction: MergePrediction,
  mode: "merge" | "rebase",
  branch: string,
  current: string,
): PredictionSummary | null {
  switch (prediction.type) {
    case "upToDate":
      return { tone: "info", text: `Already up to date. ${current} already contains ${branch}.`, files: [] };
    case "fastForward":
      return { tone: "ok", text: `${current} will fast-forward to ${branch}.`, files: [] };
    case "clean":
      return {
        tone: "ok",
        text: mode === "merge" ? "No conflicts. This merge can be done automatically." : "No conflicts expected.",
        files: [],
      };
    case "conflicts": {
      const count = plural(prediction.files.length, "file");
      return {
        tone: "warn",
        text: mode === "merge" ? `${count} will conflict.` : `${count} may conflict while replaying commits.`,
        files: prediction.files,
      };
    }
    default:
      return null;
  }
}

export function latestRequest<A, T>(
  fetch: (arg: A) => Promise<T>,
  delay: number,
  apply: (result: T | null, pending: boolean) => void,
) {
  let token = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const cancel = () => {
    token++;
    clearTimeout(timer);
  };
  const request = (arg: A | null) => {
    cancel();
    const mine = token;
    if (arg === null) return apply(null, false);
    apply(null, true);
    timer = setTimeout(() => {
      fetch(arg).then(
        (result) => mine === token && apply(result, false),
        () => mine === token && apply(null, false),
      );
    }, delay);
  };
  return { request, cancel };
}
