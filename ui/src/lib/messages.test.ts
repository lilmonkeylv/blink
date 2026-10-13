import { expect, test } from "bun:test";
import { pushRecent } from "./messages";

test("newest first, no duplicates, capped", () => {
  let list = pushRecent([], { summary: "a", description: "" });
  list = pushRecent(list, { summary: "b", description: "body " });
  list = pushRecent(list, { summary: " a ", description: "again" });
  expect(list).toEqual([
    { summary: "a", description: "again" },
    { summary: "b", description: "body" },
  ]);
  expect(pushRecent(list, { summary: "c", description: "" }, 2).map((m) => m.summary)).toEqual(["c", "a"]);
});

test("empty summaries are not saved", () => {
  expect(pushRecent([], { summary: "  ", description: "x" })).toEqual([]);
});
