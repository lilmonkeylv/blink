import { expect, test } from "bun:test";
import type { Submodule } from "../api";
import { missingSubmodules, missingSubmodulesMessage } from "./submodules";

const sub = (path: string, initialized: boolean): Submodule => ({ name: path, path, url: null, initialized });

test("lists submodules that still need a checkout", () => {
  const list = [sub("vendor/a", true), sub("vendor/b", false), sub("docs/theme", false)];
  expect(missingSubmodules(list).map((s) => s.path)).toEqual(["vendor/b", "docs/theme"]);
  expect(missingSubmodules([sub("a", true)])).toEqual([]);
});

test("words the banner for one or many submodules", () => {
  expect(missingSubmodulesMessage([sub("vendor/b", false)])).toBe("Submodule vendor/b isn't checked out yet.");
  expect(missingSubmodulesMessage([sub("a", false), sub("b", false)])).toBe("2 submodules aren't checked out yet.");
});
