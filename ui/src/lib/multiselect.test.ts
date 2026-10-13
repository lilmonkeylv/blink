import { expect, test } from "bun:test";
import { nextSelection } from "./multiselect";

const order = ["a", "b", "c", "d", "e"];
const plain = { shift: false, toggle: false };
const sorted = (s: Set<string>) => [...s].sort();

test("plain click selects one", () => {
  expect(sorted(nextSelection(order, new Set(["a", "b"]), "a", "c", plain).selected)).toEqual(["c"]);
});

test("shift click selects the range from the anchor in either direction", () => {
  expect(sorted(nextSelection(order, new Set(["b"]), "b", "d", { shift: true, toggle: false }).selected)).toEqual(["b", "c", "d"]);
  expect(sorted(nextSelection(order, new Set(["d"]), "d", "a", { shift: true, toggle: false }).selected)).toEqual(["a", "b", "c", "d"]);
});

test("ctrl click toggles but never empties the selection", () => {
  const added = nextSelection(order, new Set(["a"]), "a", "c", { shift: false, toggle: true });
  expect(sorted(added.selected)).toEqual(["a", "c"]);
  expect(added.anchor).toBe("c");
  expect(sorted(nextSelection(order, added.selected, "c", "a", { shift: false, toggle: true }).selected)).toEqual(["c"]);
  expect(sorted(nextSelection(order, new Set(["a"]), "a", "a", { shift: false, toggle: true }).selected)).toEqual(["a"]);
});

test("ctrl shift click adds the range", () => {
  expect(sorted(nextSelection(order, new Set(["a"]), "c", "e", { shift: true, toggle: true }).selected)).toEqual(["a", "c", "d", "e"]);
});
