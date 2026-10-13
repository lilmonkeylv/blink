import { describe, expect, test } from "bun:test";
import { buildManifest } from "./updater-manifest";

const base = "https://github.com/lilmonkeylv/blink/releases/download/v0.2.0";

describe("updater manifest", () => {
  test("maps every bundle to its platform keys", () => {
    const m = buildManifest(
      [
        { path: "dist/nsis/Blink_0.2.0_x64-setup.exe", signature: "sig-nsis\n" },
        { path: "dist/msi/Blink_0.2.0_x64_en-US.msi", signature: "sig-msi" },
        { path: "dist/macos/Blink.app.tar.gz", signature: "sig-mac" },
        { path: "dist/appimage/Blink_0.2.0_amd64.AppImage", signature: "sig-linux" },
        { path: "dist/deb/blink_0.2.0_amd64.deb", signature: "ignored" },
      ],
      "0.2.0",
      "notes",
      base,
      new Date("2026-10-13T00:00:00Z"),
    );
    expect(Object.keys(m.platforms).sort()).toEqual([
      "darwin-aarch64",
      "darwin-x86_64",
      "linux-x86_64",
      "linux-x86_64-appimage",
      "windows-x86_64",
      "windows-x86_64-msi",
      "windows-x86_64-nsis",
    ]);
    expect(m.platforms["windows-x86_64"]).toEqual({ url: `${base}/Blink_0.2.0_x64-setup.exe`, signature: "sig-nsis" });
    expect(m.platforms["darwin-aarch64"].url).toBe(`${base}/Blink.app.tar.gz`);
    expect(m.pub_date).toBe("2026-10-13T00:00:00.000Z");
  });

  test("encodes file names with spaces", () => {
    const m = buildManifest([{ path: "x/My App.AppImage", signature: "s" }], "1.0.0", "", base);
    expect(m.platforms["linux-x86_64"].url).toBe(`${base}/My%20App.AppImage`);
  });
});
