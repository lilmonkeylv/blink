import { expect, test } from "bun:test";
import { transferPercent } from "./progress";

const p = (phase: string, percent: number | null) => ({ phase, percent, line: "" });

test("only real transfer phases drive the bar", () => {
  expect(transferPercent(p("Receiving objects", 45))).toBe(45);
  expect(transferPercent(p("Resolving deltas", 10))).toBe(10);
  expect(transferPercent(p("Writing objects", 70))).toBe(70);
  expect(transferPercent(p("Counting objects", 100))).toBeNull();
  expect(transferPercent(p("Compressing objects", 40))).toBeNull();
  expect(transferPercent(p("Receiving objects", 100))).toBeNull();
  expect(transferPercent(p("From https://github.com/a/b", null))).toBeNull();
  expect(transferPercent(null)).toBeNull();
});
