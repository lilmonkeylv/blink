import { expect, test } from "bun:test";
import { cloneUrl, isShorthand, repoNameFromUrl } from "./clone";

test("owner/name expands to a GitHub URL", () => {
  expect(cloneUrl("lilmonkeylv/blink")).toBe("https://github.com/lilmonkeylv/blink.git");
  expect(cloneUrl("  lilmonkeylv/blink.git ")).toBe("https://github.com/lilmonkeylv/blink.git");
  expect(cloneUrl("a.b/c-d_e")).toBe("https://github.com/a.b/c-d_e.git");
});

test("full URLs and partial input pass through trimmed", () => {
  expect(cloneUrl(" https://gitlab.com/a/b.git ")).toBe("https://gitlab.com/a/b.git");
  expect(cloneUrl("git@github.com:a/b.git")).toBe("git@github.com:a/b.git");
  expect(cloneUrl("lilmonkeylv/")).toBe("lilmonkeylv/");
  expect(cloneUrl("lilmonkeylv")).toBe("lilmonkeylv");
  expect(cloneUrl("")).toBe("");
});

test("shorthand detection ignores paths and URLs", () => {
  expect(isShorthand("a/b")).toBe(true);
  expect(isShorthand("https://github.com/a/b")).toBe(false);
  expect(isShorthand("a/b/c")).toBe(false);
  expect(isShorthand("a b/c")).toBe(false);
});

test("folder name comes from the last path part", () => {
  expect(repoNameFromUrl("https://github.com/a/blink.git")).toBe("blink");
  expect(repoNameFromUrl("git@github.com:a/blink.git")).toBe("blink");
  expect(repoNameFromUrl("a/blink/")).toBe("blink");
});
