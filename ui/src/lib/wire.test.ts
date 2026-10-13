import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { decodeDiff, decodeGraph, decodeHistory } from "./wire";

const fixture = (name: string) => readFileSync(join(import.meta.dir, "fixtures", name));

test("decodes the diff written by the Rust encoder", () => {
  expect(decodeDiff(fixture("diff.bin"))).toEqual({
    binary: false,
    tooLarge: false,
    additions: 1,
    deletions: 1,
    hunks: [
      {
        oldStart: 10,
        oldLen: 2,
        newStart: 10,
        newLen: 2,
        lines: [
          { kind: "context", oldNo: 10, newNo: 10, text: "fn main() {", noNewline: false, tokens: [[0, 2, 5]] },
          { kind: "removed", oldNo: 11, newNo: null, text: "    old();", noNewline: false },
          { kind: "added", oldNo: null, newNo: 11, text: "let é = \"🙂\";", noNewline: true, tokens: [[4, 1, 8], [8, 4, 2]] },
        ],
      },
    ],
  });
});

test("decodes LFS details carried as JSON", () => {
  const diff = decodeDiff(fixture("lfs.bin"));
  expect(diff.hunks).toEqual([]);
  expect(diff.lfs).toEqual({ old: null, new: { oid: "abc", size: 42 } });
  expect(diff.submodule).toBeUndefined();
});

test("decodes graph pages", () => {
  const page = decodeGraph(fixture("graph.bin"));
  expect(page.commits.map((c) => c.summary)).toEqual(["Commit b: ünïcode", "Commit a: ünïcode"]);
  expect(page.commits[0]).toMatchObject({ id: "b".repeat(40), shortId: "b".repeat(7), time: 1_760_000_000, parents: ["a".repeat(40)] });
  expect(page.commits[1].parents).toEqual([]);
  expect(page.rows).toEqual([
    { lane: 0, color: 0, width: 1, top: [], bottom: [{ from: 0, to: 0, color: 0 }] },
    { lane: 0, color: 0, width: 1, top: [{ from: 0, to: 0, color: 0 }], bottom: [] },
  ]);
});

test("rejects a payload of the wrong kind", () => {
  expect(() => decodeHistory(fixture("diff.bin"))).toThrow();
});

test("accepts the number array the IPC fallback produces", () => {
  expect(decodeDiff(Array.from(fixture("lfs.bin"))).lfs?.new?.size).toBe(42);
});
