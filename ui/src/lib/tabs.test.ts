import { expect, test } from "bun:test";
import { closeTab, cycleTab, placeTab } from "./tabs";

test("opening replaces the active tab unless asked for a new one", () => {
  expect(placeTab([], null, "a", false)).toEqual(["a"]);
  expect(placeTab(["a", "b"], "a", "c", false)).toEqual(["c", "b"]);
  expect(placeTab(["a", "b"], "a", "c", true)).toEqual(["a", "c", "b"]);
  expect(placeTab(["a", "b"], "a", "b", false)).toEqual(["a", "b"]);
});

test("closing the active tab moves to its neighbour", () => {
  expect(closeTab(["a", "b", "c"], "b", "b")).toEqual({ tabs: ["a", "c"], next: "c" });
  expect(closeTab(["a", "b", "c"], "c", "c")).toEqual({ tabs: ["a", "b"], next: "b" });
  expect(closeTab(["a", "b"], "a", "b")).toEqual({ tabs: ["b"], next: "b" });
  expect(closeTab(["a"], "a", "a")).toEqual({ tabs: [], next: null });
});

test("cycling wraps around", () => {
  expect(cycleTab(["a", "b", "c"], "c", 1)).toBe("a");
  expect(cycleTab(["a", "b", "c"], "a", -1)).toBe("c");
  expect(cycleTab([], null, 1)).toBeNull();
});
