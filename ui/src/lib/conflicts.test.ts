import { describe, expect, test } from "bun:test";
import { conflictCount, parseConflicts, render } from "./conflicts";

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

  test("unterminated markers stay as text", () => {
    const segments = parseConflicts("<<<<<<< HEAD\nonly");
    expect(conflictCount(segments)).toBe(0);
  });
});
