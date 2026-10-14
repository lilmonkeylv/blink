import { describe, expect, test } from "bun:test";
import { baseSections, conflictCount, parseConflicts, render } from "./conflicts";

const text = ["head", "<<<<<<< HEAD", "ours 1", "ours 2", "=======", "theirs", ">>>>>>> feature", "tail", ""].join("\n");

describe("conflicts", () => {
  test("parses a conflict between plain text", () => {
    const segments = parseConflicts(text);
    expect(segments).toHaveLength(3);
    expect(conflictCount(segments)).toBe(1);
    const c = segments[1];
    if (c.type !== "conflict") throw new Error("expected conflict");
    expect(c.ours).toEqual(["ours 1", "ours 2"]);
    expect(c.theirs).toEqual(["theirs"]);
    expect(c.theirsLabel).toBe("feature");
  });

  test("renders resolutions", () => {
    const segments = parseConflicts(text);
    expect(render(segments, ["ours"]).text).toBe("head\nours 1\nours 2\ntail\n");
    expect(render(segments, ["theirs"]).text).toBe("head\ntheirs\ntail\n");
    expect(render(segments, ["both"]).text).toBe("head\nours 1\nours 2\ntheirs\ntail\n");
    const pending = render(segments, [null]);
    expect(pending.unresolved).toBe(1);
    expect(pending.text).toContain("<<<<<<< HEAD");
  });

  test("handles diff3 base sections", () => {
    const diff3 = ["<<<<<<< ours", "a", "||||||| base", "o", "=======", "b", ">>>>>>> theirs"].join("\n");
    const [c] = parseConflicts(diff3);
    if (c.type !== "conflict") throw new Error("expected conflict");
    expect(c.base).toEqual(["o"]);
    expect(c.ours).toEqual(["a"]);
  });

  test("finds each conflict's base lines between anchors", () => {
    const merged = [
      "fn a() {",
      "}",
      "fn b() {",
      "<<<<<<< HEAD",
      "    run(64);",
      "=======",
      "    run(256);",
      ">>>>>>> feature",
      "}",
      "fn c() {",
      "<<<<<<< HEAD",
      "    x",
      "=======",
      "    y",
      ">>>>>>> feature",
      "}",
      "",
    ].join("\n");
    const base = ["fn a() {", "}", "fn b() {", "    run(8);", "    flush();", "}", "fn c() {", "    w", "}", ""].join("\r\n");
    expect(baseSections(parseConflicts(merged), base)).toEqual([["    run(8);", "    flush();"], ["    w"]]);
  });

  test("prefers diff3 base and reports conflicts it cannot place", () => {
    const merged = ["start", "<<<<<<< ours", "a", "=======", "b", ">>>>>>> theirs", "gone", "<<<<<<< ours", "c", "||||||| base", "o", "=======", "d", ">>>>>>> theirs"].join("\n");
    expect(baseSections(parseConflicts(merged), "other\nfile")).toEqual([null, ["o"]]);
  });

  test("handles a conflict at the end of the file", () => {
    const merged = ["keep", "<<<<<<< ours", "a", "=======", "b", ">>>>>>> theirs", ""].join("\n");
    expect(baseSections(parseConflicts(merged), "keep\nold\n")).toEqual([["old"]]);
  });

  test("unterminated markers stay as text", () => {
    const segments = parseConflicts("<<<<<<< HEAD\nonly");
    expect(conflictCount(segments)).toBe(0);
  });
});
