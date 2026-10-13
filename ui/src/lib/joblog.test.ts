import { expect, test } from "bun:test";
import { parseJobLog } from "./joblog";

const LOG = [
  "\uFEFF2026-10-13T08:00:00.1234567Z ##[group]Run cargo test",
  "2026-10-13T08:00:00.2000000Z cargo test --workspace",
  "2026-10-13T08:00:01.0000000Z ##[endgroup]",
  "2026-10-13T08:00:02.0000000Z test result: 0 errors in setup",
  "2026-10-13T08:00:03.0000000Z thread 'main' panicked",
  "2026-10-13T08:00:04.0000000Z ##[error]Process completed with exit code 101.",
  "",
].join("\r\n");

test("strips timestamps and markers and finds the first error", () => {
  const log = parseJobLog(LOG);
  expect(log.hidden).toBe(0);
  expect(log.lines.map((l) => l.text)).toEqual([
    "Run cargo test",
    "cargo test --workspace",
    "test result: 0 errors in setup",
    "thread 'main' panicked",
    "Process completed with exit code 101.",
  ]);
  expect(log.lines[0].kind).toBe("group");
  expect(log.firstError).toBe(4);
  expect(log.lines[4].kind).toBe("error");
});

test("falls back to a plain error word when there is no error marker", () => {
  const log = parseJobLog("2026-10-13T08:00:00Z ok\n2026-10-13T08:00:01Z error: linker failed\n");
  expect(log.firstError).toBe(1);
  expect(log.lines[1].text).toBe("error: linker failed");
  expect(parseJobLog("all good\n").firstError).toBe(-1);
});

test("keeps only the last lines past the limit", () => {
  const log = parseJobLog(Array.from({ length: 12 }, (_, i) => `line ${i}`).join("\n"), 5);
  expect(log.hidden).toBe(7);
  expect(log.lines.map((l) => l.text)).toEqual(["line 7", "line 8", "line 9", "line 10", "line 11"]);
});
