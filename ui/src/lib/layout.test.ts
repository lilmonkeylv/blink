import { expect, test } from "bun:test";
import { clampSidebar, mergeLayout } from "./layout";

test("merging keeps earlier fields and moves the repo to the end", () => {
  const all = { a: { tab: "history" as const, sidebar: 300 }, b: { sidebar: 280 } };
  const next = mergeLayout(all, "a", { selectedPath: "x.ts" });
  expect(Object.keys(next)).toEqual(["b", "a"]);
  expect(next.a).toEqual({ tab: "history", sidebar: 300, selectedPath: "x.ts" });
});

test("old repos fall off past the cap", () => {
  const all = { a: {}, b: {}, c: {} };
  expect(Object.keys(mergeLayout(all, "d", {}, 3))).toEqual(["b", "c", "d"]);
});

test("sidebar width stays usable", () => {
  expect(clampSidebar(100, 1400)).toBe(220);
  expect(clampSidebar(900, 1400)).toBe(640);
  expect(clampSidebar(500, 800)).toBe(440);
});
