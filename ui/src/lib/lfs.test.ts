import { expect, test } from "bun:test";
import { claimMissingLfsWarning, lfsPatternFor } from "./lfs";

test("suggests a pattern from the file extension", () => {
  expect(lfsPatternFor("art/hero.psd")).toBe("*.psd");
  expect(lfsPatternFor("model.tar.gz")).toBe("*.gz");
  expect(lfsPatternFor("bin/tool")).toBe("bin/tool");
  expect(lfsPatternFor(".env")).toBe(".env");
  expect(lfsPatternFor("trailing.")).toBe("trailing.");
});

test("warns about missing LFS once per repository", () => {
  const data = new Map<string, string>();
  const storage = { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
  expect(claimMissingLfsWarning("C:/dev/a", storage)).toBe(true);
  expect(claimMissingLfsWarning("C:/dev/a", storage)).toBe(false);
  expect(claimMissingLfsWarning("C:/dev/b", storage)).toBe(true);
  data.set("blink.lfsMissingWarned", "not json");
  expect(claimMissingLfsWarning("C:/dev/a", storage)).toBe(true);
});
