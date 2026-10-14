import { expect, test } from "bun:test";
import { applyHookEvent, failedHook, hookTitle, MAX_HOOK_LINES } from "./hooks";

const repo = "C:/dev/blink";

test("collects output for the running hook and settles on the result", () => {
  let run = applyHookEvent(null, repo, "commit", { type: "started", hooks: ["pre-commit", "commit-msg"] });
  expect(hookTitle(run!)).toBe("Running pre-commit, commit-msg");
  run = applyHookEvent(run, repo, "commit", { type: "output", line: "lint: ok" });
  run = applyHookEvent(run, repo, "commit", { type: "output", line: "src/a.rs:1: error" });
  run = applyHookEvent(run, repo, "commit", {
    type: "finished",
    results: [
      { name: "pre-commit", code: 0 },
      { name: "commit-msg", code: 1 },
    ],
  });
  expect(run!.lines).toEqual(["lint: ok", "src/a.rs:1: error"]);
  expect(run!.status).toBe("failed");
  expect(failedHook(run!)?.name).toBe("commit-msg");
  expect(hookTitle(run!)).toBe("commit-msg hook failed (exit 1)");
});

test("passing runs summarise every hook", () => {
  let run = applyHookEvent(null, repo, "push", { type: "started", hooks: ["pre-push"] });
  run = applyHookEvent(run, repo, "push", { type: "finished", results: [{ name: "pre-push", code: 0 }] });
  expect(run!.status).toBe("passed");
  expect(hookTitle(run!)).toBe("pre-push hook passed");
});

test("ignores output from another repository or action", () => {
  const run = applyHookEvent(null, repo, "commit", { type: "started", hooks: ["pre-commit"] });
  expect(applyHookEvent(run, "C:/other", "commit", { type: "output", line: "x" })).toBe(run);
  expect(applyHookEvent(run, repo, "push", { type: "output", line: "x" })).toBe(run);
});

test("a result without a start still shows up", () => {
  const run = applyHookEvent(null, repo, "commit", { type: "finished", results: [{ name: "pre-commit", code: 2 }] });
  expect(run!.hooks).toEqual(["pre-commit"]);
  expect(run!.status).toBe("failed");
});

test("keeps only the newest lines of very chatty hooks", () => {
  let run = applyHookEvent(null, repo, "commit", { type: "started", hooks: ["pre-commit"] });
  for (let i = 0; i < MAX_HOOK_LINES + 25; i++) run = applyHookEvent(run, repo, "commit", { type: "output", line: `line ${i}` });
  expect(run!.lines.length).toBe(MAX_HOOK_LINES);
  expect(run!.dropped).toBe(25);
  expect(run!.lines[0]).toBe("line 25");
});
