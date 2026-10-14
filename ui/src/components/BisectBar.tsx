import { Show } from "solid-js";
import { api, type BisectMark } from "../api";
import { bisectPhase, bisectStatus } from "../lib/bisect";
import { notify, refreshRepo, run, setStore, store } from "../store";

export async function startBisect(bad: string) {
  const repo = store.repo;
  if (!repo) return;
  const state = await run("Start bisect", () => api.bisectStart(repo.id, bad));
  if (state === undefined) return;
  setStore("bisect", state);
  await refreshRepo();
}

export async function markBisect(mark: BisectMark, sha?: string) {
  const repo = store.repo;
  if (!repo) return;
  const state = await run("Bisect", () => api.bisectMark(repo.id, mark, sha));
  if (state === undefined) return;
  setStore("bisect", state);
  if (state?.firstBad) notify("success", `Found the first bad commit: ${state.firstBad.slice(0, 7)}`);
  await refreshRepo();
}

export async function resetBisect() {
  const repo = store.repo;
  if (!repo) return;
  const done = await run("Reset bisect", () => api.bisectReset(repo.id).then(() => true));
  if (!done) return;
  setStore("bisect", null);
  await refreshRepo();
}

export function BisectBar() {
  const state = () => store.bisect;
  return (
    <Show when={state()}>
      {(s) => (
        <div class="operation-bar stacked bisect-bar">
          <span class="operation-status" title={s().current ?? undefined}>
            {bisectStatus(s())}
          </span>
          <div class="operation-actions">
            <Show when={bisectPhase(s()) === "testing"}>
              <button class="btn" onClick={() => markBisect("good")}>
                Good
              </button>
              <button class="btn" onClick={() => markBisect("bad")}>
                Bad
              </button>
              <button class="btn" onClick={() => markBisect("skip")}>
                Skip
              </button>
            </Show>
            <Show when={bisectPhase(s()) === "found" && s().firstBad}>
              {(sha) => (
                <button class="btn" onClick={() => navigator.clipboard.writeText(sha())}>
                  Copy SHA
                </button>
              )}
            </Show>
            <button class="btn" classList={{ primary: bisectPhase(s()) === "found" }} onClick={resetBisect}>
              {bisectPhase(s()) === "found" ? "Finish" : "Reset"}
            </button>
          </div>
        </div>
      )}
    </Show>
  );
}
