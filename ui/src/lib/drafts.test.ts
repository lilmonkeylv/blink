import { expect, test } from "bun:test";
import { EMPTY_DRAFT, parseDrafts, withDraft } from "./drafts";

test("drafts are stored per repo and blank ones are dropped", () => {
  const a = { ...EMPTY_DRAFT, summary: "fix a" };
  const b = { ...EMPTY_DRAFT, amend: true };
  const all = withDraft(withDraft({}, "/a", a), "/b", b);
  expect(all).toEqual({ "/a": a, "/b": b });
  expect(withDraft(all, "/a", EMPTY_DRAFT)).toEqual({ "/b": b });
  expect(all["/a"]).toEqual(a);
});

test("saved drafts survive bad or partial data", () => {
  expect(parseDrafts(null)).toEqual({});
  expect(parseDrafts("not json")).toEqual({});
  expect(parseDrafts("[1,2]")).toEqual({});
  const raw = JSON.stringify({ "/a": { summary: "wip", amend: "yes" }, "/b": 3, "/c": { summary: "" } });
  expect(parseDrafts(raw)).toEqual({ "/a": { ...EMPTY_DRAFT, summary: "wip" } });
});
