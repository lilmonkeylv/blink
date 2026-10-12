import { describe, expect, test } from "bun:test";
import type { FileDiff } from "../api";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";
import { checkState, defaultSel, lineKey, reconcile, setLines, toggleFile, toPayload } from "./selection";
import { relativeTime } from "./time";

const diff: FileDiff = {
  binary: false,
  tooLarge: false,
  additions: 2,
  deletions: 1,
  hunks: [
    {
      oldStart: 1,
      oldLen: 2,
      newStart: 1,
      newLen: 3,
      lines: [
        { kind: "context", oldNo: 1, newNo: 1, text: "a", noNewline: false },
        { kind: "removed", oldNo: 2, newNo: null, text: "b", noNewline: false },
        { kind: "added", oldNo: null, newNo: 2, text: "B", noNewline: false },
        { kind: "added", oldNo: null, newNo: 3, text: "C", noNewline: false },
      ],
    },
  ],
};

describe("selection", () => {
  test("default includes whole file", () => {
    expect(checkState(undefined, diff)).toBe("all");
    expect(toPayload("f", undefined, diff)).toEqual({ path: "f" });
  });

  test("excluding a line yields partial payload", () => {
    const sel = setLines(defaultSel(), [lineKey(0, 3)], false, diff);
    expect(checkState(sel, diff)).toBe("partial");
    expect(toPayload("f", sel, diff)).toEqual({ path: "f", lines: [[0, 1], [0, 2]] });
  });

  test("excluding every line unchecks file", () => {
    const sel = setLines(defaultSel(), [lineKey(0, 1), lineKey(0, 2), lineKey(0, 3)], false, diff);
    expect(checkState(sel, diff)).toBe("none");
    expect(toPayload("f", sel, diff)).toBeNull();
  });

  test("including a line on an unchecked file selects only that line", () => {
    const sel = setLines({ included: false, excluded: new Set() }, [lineKey(0, 2)], true, diff);
    expect(toPayload("f", sel, diff)).toEqual({ path: "f", lines: [[0, 2]] });
  });

  test("toggle: partial to all, all to none, none to all", () => {
    const partial = setLines(defaultSel(), [lineKey(0, 3)], false, diff);
    const all = toggleFile(partial, diff);
    expect(checkState(all, diff)).toBe("all");
    const none = toggleFile(all, diff);
    expect(checkState(none, diff)).toBe("none");
    expect(checkState(toggleFile(none, diff), diff)).toBe("all");
  });

  test("reconcile drops stale line selection", () => {
    const sel = setLines(defaultSel(), [lineKey(0, 3)], false, diff);
    const changed: FileDiff = { ...diff, additions: 5 };
    expect(reconcile(sel, changed)?.excluded.size).toBe(0);
    expect(reconcile(sel, diff)).toBe(sel);
  });
});

describe("fuzzy", () => {
  test("substring beats scattered", () => {
    expect(fuzzyScore("main", "main")!).toBeGreaterThan(fuzzyScore("main", "my-awesome-index")!);
    expect(fuzzyScore("xyz", "main")).toBeNull();
  });

  test("filters and ranks", () => {
    const out = fuzzyFilter(["feature/login", "main", "fix/logout"], "log", (s) => s);
    expect(out).toEqual(["fix/logout", "feature/login"]);
  });
});

describe("time", () => {
  test("relative", () => {
    const now = 1_000_000;
    expect(relativeTime(now - 10, now)).toBe("just now");
    expect(relativeTime(now - 3 * 3600, now)).toBe("3 hours ago");
    expect(relativeTime(now - 86400, now)).toBe("yesterday");
  });
});
