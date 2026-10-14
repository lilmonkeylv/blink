import { expect, test } from "bun:test";
import type { BisectState } from "../api";
import { bisectPhase, bisectRole, bisectStatus } from "./bisect";

const sha = (c: string) => c.repeat(40);

const base: BisectState = {
  bad: sha("b"),
  good: [],
  skipped: [],
  current: sha("c"),
  currentSummary: "Tune watcher",
  revisionsLeft: null,
  stepsLeft: null,
  firstBad: null,
  firstBadSummary: null,
};

test("waits for a good commit after starting", () => {
  expect(bisectPhase(base)).toBe("waiting");
  expect(bisectStatus(base)).toBe("Bisecting: bbbbbbb is bad. Mark a commit you know is good in History.");
});

test("reports the commit under test and the steps left", () => {
  const testing = { ...base, good: [sha("a")], revisionsLeft: 6, stepsLeft: 3 };
  expect(bisectPhase(testing)).toBe("testing");
  expect(bisectStatus(testing)).toBe("Bisecting: testing ccccccc Tune watcher, about 3 steps left");
  expect(bisectStatus({ ...testing, stepsLeft: 1 })).toContain("about 1 step left");
});

test("names the first bad commit once found", () => {
  const found = { ...base, good: [sha("a")], firstBad: sha("d"), firstBadSummary: "Break the watcher" };
  expect(bisectPhase(found)).toBe("found");
  expect(bisectStatus(found)).toBe("ddddddd is the first bad commit: Break the watcher");
});

test("tags commits in history by their bisect role", () => {
  const state = { ...base, good: [sha("a")], skipped: [sha("e")] };
  expect(bisectRole(state, sha("b"))).toBe("bad");
  expect(bisectRole(state, sha("a"))).toBe("good");
  expect(bisectRole(state, sha("e"))).toBe("skipped");
  expect(bisectRole(state, sha("c"))).toBe("current");
  expect(bisectRole(state, sha("f"))).toBeNull();
  expect(bisectRole(null, sha("b"))).toBeNull();
});
