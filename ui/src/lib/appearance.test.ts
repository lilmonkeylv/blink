import { expect, test } from "bun:test";
import { ACCENT_PRESETS, DEFAULT_MONO, accentTokens, listRowHeight, monoStack, normalizeHex, readableOn } from "./appearance";

test("hex colors are normalised or rejected", () => {
  expect(normalizeHex("#2F6FEB")).toBe("#2f6feb");
  expect(normalizeHex(" abc ")).toBe("#aabbcc");
  expect(normalizeHex("#12345")).toBeNull();
  expect(normalizeHex("red")).toBeNull();
  expect(normalizeHex(null)).toBeNull();
});

test("accent text stays readable", () => {
  expect(readableOn("#2f6feb")).toBe("#ffffff");
  expect(readableOn("#ffd33d")).toBe("#1f2328");
  expect(readableOn("#ffffff")).toBe("#1f2328");
  for (const preset of ACCENT_PRESETS) expect(readableOn(preset.value)).toBe("#ffffff");
});

test("accent tokens cover the selection colors", () => {
  expect(accentTokens(null)).toBeNull();
  expect(accentTokens("nope")).toBeNull();
  const tokens = accentTokens("#8250DF")!;
  expect(tokens["--accent"]).toBe("#8250df");
  expect(tokens["--bg-selected-focus"]).toBe("#8250df");
  expect(tokens["--accent-fg"]).toBe("#ffffff");
  expect(tokens["--bg-selected"]).toContain("#8250df");
});

test("diff fonts fall back to the default stack", () => {
  expect(monoStack(null)).toBeNull();
  expect(monoStack("  ")).toBeNull();
  expect(monoStack("JetBrains Mono")).toBe(`"JetBrains Mono", ${DEFAULT_MONO}`);
  expect(monoStack("Fira Code, monospace")).toBe(`"Fira Code", monospace, ${DEFAULT_MONO}`);
  expect(monoStack(`'Iosevka Term'`)).toBe(`'Iosevka Term', ${DEFAULT_MONO}`);
});

test("larger hit targets win over density", () => {
  expect(listRowHeight("comfortable", false)).toBe(30);
  expect(listRowHeight("compact", false)).toBe(24);
  expect(listRowHeight("compact", true)).toBe(36);
  expect(listRowHeight(undefined, undefined)).toBe(30);
});
