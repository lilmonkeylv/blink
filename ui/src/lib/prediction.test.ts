import { expect, test } from "bun:test";
import { describePrediction, latestRequest } from "./prediction";

test("describes each merge outcome", () => {
  expect(describePrediction({ type: "upToDate" }, "merge", "feature", "main")?.text).toBe("Already up to date. main already contains feature.");
  expect(describePrediction({ type: "fastForward" }, "merge", "feature", "main")?.text).toBe("main will fast-forward to feature.");
  expect(describePrediction({ type: "clean" }, "merge", "feature", "main")?.tone).toBe("ok");
  const conflicts = describePrediction({ type: "conflicts", files: ["a.rs", "b.rs"] }, "merge", "feature", "main");
  expect(conflicts).toEqual({ tone: "warn", text: "2 files will conflict.", files: ["a.rs", "b.rs"] });
  expect(describePrediction({ type: "conflicts", files: ["a.rs"] }, "rebase", "main", "feature")?.text).toBe(
    "1 file may conflict while replaying commits.",
  );
  expect(describePrediction({ type: "unknown" }, "merge", "feature", "main")).toBeNull();
});

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

test("only the newest request is applied", async () => {
  const seen: [string | null, boolean][] = [];
  const calls: string[] = [];
  const slow = (arg: string) => {
    calls.push(arg);
    return wait(arg === "first" ? 40 : 5).then(() => arg);
  };
  const latest = latestRequest(slow, 10, (result, pending) => seen.push([result, pending]));
  latest.request("ignored");
  latest.request("first");
  await wait(15);
  latest.request("second");
  await wait(80);
  expect(calls).toEqual(["first", "second"]);
  expect(seen.filter(([result]) => result !== null)).toEqual([["second", false]]);
  expect(seen.at(-1)).toEqual(["second", false]);
});

test("clearing the request resets without fetching", async () => {
  const seen: [number | null, boolean][] = [];
  let fetched = 0;
  const latest = latestRequest(
    () => Promise.resolve(++fetched),
    5,
    (result, pending) => seen.push([result, pending]),
  );
  latest.request(1);
  latest.request(null);
  await wait(20);
  expect(fetched).toBe(0);
  expect(seen.at(-1)).toEqual([null, false]);
});
