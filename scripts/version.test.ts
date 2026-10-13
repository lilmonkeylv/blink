import { describe, expect, test } from "bun:test";
import { cargoVersion, versionFromTag, withCargoVersion, withPackageVersion } from "./version";

const toml = `[workspace]
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
gix = { version = "0.89" }
`;

describe("version", () => {
  test("reads the workspace version only", () => {
    expect(cargoVersion(toml)).toBe("0.1.0");
  });

  test("rewrites the workspace version without touching dependencies", () => {
    const out = withCargoVersion(toml, "0.2.0-beta.1");
    expect(cargoVersion(out)).toBe("0.2.0-beta.1");
    expect(out).toContain('gix = { version = "0.89" }');
  });

  test("rewrites package.json", () => {
    expect(JSON.parse(withPackageVersion('{"name":"blink","version":"0.1.0"}', "1.0.0")).version).toBe("1.0.0");
  });

  test("strips tag prefixes", () => {
    expect(versionFromTag("v0.3.0")).toBe("0.3.0");
    expect(versionFromTag("refs/tags/v0.3.0-rc.1")).toBe("0.3.0-rc.1");
  });
});
