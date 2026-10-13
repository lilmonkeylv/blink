import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

export interface Platform {
  url: string;
  signature: string;
}

export interface Manifest {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Record<string, Platform>;
}

const KINDS: { match: (name: string) => boolean; keys: string[] }[] = [
  { match: (n) => n.endsWith("-setup.exe"), keys: ["windows-x86_64", "windows-x86_64-nsis"] },
  { match: (n) => n.endsWith(".msi"), keys: ["windows-x86_64-msi"] },
  { match: (n) => n.endsWith(".app.tar.gz"), keys: ["darwin-aarch64", "darwin-x86_64"] },
  { match: (n) => n.endsWith(".AppImage"), keys: ["linux-x86_64", "linux-x86_64-appimage"] },
];

export function channelFiles(version: string): string[] {
  return version.includes("-") ? ["beta.json"] : ["latest.json", "beta.json"];
}

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((entry) => {
    const full = join(dir, entry);
    return statSync(full).isDirectory() ? walk(full) : [full];
  });
}

export function buildManifest(files: { path: string; signature: string }[], version: string, notes: string, baseUrl: string, date = new Date()): Manifest {
  const platforms: Record<string, Platform> = {};
  for (const { path, signature } of files) {
    const name = basename(path);
    const kind = KINDS.find((k) => k.match(name));
    if (!kind) continue;
    for (const key of kind.keys) {
      platforms[key] ??= { url: `${baseUrl}/${encodeURIComponent(name)}`, signature: signature.trim() };
    }
  }
  return { version, notes, pub_date: date.toISOString(), platforms };
}

if (import.meta.main) {
  const [dir, version, tag, notesFile, outDir = "."] = process.argv.slice(2);
  if (!dir || !version || !tag) {
    console.error("usage: bun scripts/updater-manifest.ts <dist dir> <version> <tag> [notes.md] [out dir]");
    process.exit(1);
  }
  const signed = walk(dir)
    .filter((f) => f.endsWith(".sig"))
    .map((sig) => ({ path: sig.slice(0, -4), signature: readFileSync(sig, "utf8") }));
  if (!signed.length) {
    console.log("no updater signatures found, skipping updater manifests");
    process.exit(0);
  }
  const notes = notesFile ? readFileSync(notesFile, "utf8").trim() : "";
  const manifest = buildManifest(signed, version, notes, `https://github.com/lilmonkeylv/blink/releases/download/${tag}`);
  const body = `${JSON.stringify(manifest, null, 2)}\n`;
  for (const name of channelFiles(version)) {
    writeFileSync(join(outDir, name), body);
    console.log(`wrote ${join(outDir, name)} with ${Object.keys(manifest.platforms).join(", ")}`);
  }
}
