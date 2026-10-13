import { expect, test } from "bun:test";
import { comboFromEvent, normalizeCombo, resolveBindings } from "./keymap";

const press = (key: string, mods: Partial<{ ctrl: boolean; meta: boolean; alt: boolean; shift: boolean }> = {}) => ({
  key,
  ctrlKey: !!mods.ctrl,
  metaKey: !!mods.meta,
  altKey: !!mods.alt,
  shiftKey: !!mods.shift,
});

test("combos normalize regardless of order and spelling", () => {
  expect(normalizeCombo("Shift+Ctrl+N")).toBe("ctrl+shift+n");
  expect(normalizeCombo("Cmd+Option+t")).toBe("ctrl+alt+t");
  expect(normalizeCombo("F5")).toBe("f5");
  expect(normalizeCombo("Ctrl+Shift")).toBeNull();
  expect(normalizeCombo("Ctrl+A+B")).toBeNull();
});

test("events map onto the same form, Cmd counts as Ctrl", () => {
  expect(comboFromEvent(press("N", { ctrl: true, shift: true }))).toBe("ctrl+shift+n");
  expect(comboFromEvent(press("t", { meta: true, alt: true }))).toBe("ctrl+alt+t");
  expect(comboFromEvent(press("Tab", { ctrl: true }))).toBe("ctrl+tab");
});

test("overrides replace defaults and an empty string unbinds", () => {
  const { byCombo, keys } = resolveBindings({ "sync.push": "Ctrl+Alt+P", "repo.create": "" });
  expect(byCombo.get("ctrl+alt+p")).toBe("sync.push");
  expect(byCombo.get("ctrl+p")).toBeUndefined();
  expect(byCombo.get("ctrl+n")).toBeUndefined();
  expect(keys["repo.create"]).toBe("");
  expect(byCombo.get("ctrl+shift+n")).toBe("branch.new");
});
