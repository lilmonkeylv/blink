import { expect, test } from "bun:test";
import type { DiffHunk } from "../api";
import { pairLines } from "./split";

const line = (kind: "context" | "added" | "removed", text: string) => ({
  kind,
  oldNo: kind === "added" ? null : 1,
  newNo: kind === "removed" ? null : 1,
  text,
  noNewline: false,
});

test("pairs removals with additions and keeps context on both sides", () => {
  const hunk: DiffHunk = {
    oldStart: 1,
    oldLen: 4,
    newStart: 1,
    newLen: 3,
    lines: [line("context", "a"), line("removed", "b"), line("removed", "c"), line("added", "B"), line("context", "d"), line("added", "e")],
  };
  const rows = pairLines([hunk]);
  const pairs = rows.filter((r) => r.type === "pair");
  expect(rows[0].type).toBe("hunk");
  expect(pairs.map((r) => [r.left?.data.text ?? null, r.right?.data.text ?? null])).toEqual([
    ["a", "a"],
    ["b", "B"],
    ["c", null],
    ["d", "d"],
    [null, "e"],
  ]);
  expect(pairs[1].left?.key).toBe("0:1");
  expect(pairs[1].right?.key).toBe("0:3");
});
