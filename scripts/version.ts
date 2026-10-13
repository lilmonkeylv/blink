import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const cargoPath = join(root, "Cargo.toml");
const packagePath = join(root, "package.json");

const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
const CARGO_VERSION = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")([^"]+)(")/;

export function cargoVersion(toml: string): string | null {
  return toml.match(CARGO_VERSION)?.[2] ?? null;
}

export function withCargoVersion(toml: string, version: string): string {
  if (!CARGO_VERSION.test(toml)) throw new Error("no version in [workspace.package]");
  return toml.replace(CARGO_VERSION, `$1${version}$3`);
}

export function withPackageVersion(json: string, version: string): string {
  const pkg = JSON.parse(json);
  pkg.version = version;
  return `${JSON.stringify(pkg, null, 2)}\n`;
}

export function versionFromTag(tag: string): string {
  return tag.replace(/^refs\/tags\//, "").replace(/^v/, "");
}

function read() {
  const cargo = cargoVersion(readFileSync(cargoPath, "utf8"));
  const pkg = JSON.parse(readFileSync(packagePath, "utf8")).version as string;
  return { cargo, pkg };
}

function check(tag?: string) {
  const { cargo, pkg } = read();
  if (cargo !== pkg) {
    console.error(`version mismatch: Cargo.toml has ${cargo}, package.json has ${pkg}`);
    process.exit(1);
  }
  if (tag && versionFromTag(tag) !== pkg) {
    console.error(`tag ${tag} does not match version ${pkg}`);
    process.exit(1);
  }
  console.log(pkg);
}

async function set(version: string | undefined) {
  if (!version || !SEMVER.test(version)) {
    console.error("usage: bun scripts/version.ts set <x.y.z[-pre]>");
    process.exit(1);
  }
  const dirty = (await Bun.$`git status --porcelain`.cwd(root).text()).trim();
  if (dirty) {
    console.error("working tree is not clean, commit or stash first");
    process.exit(1);
  }
  writeFileSync(cargoPath, withCargoVersion(readFileSync(cargoPath, "utf8"), version));
  writeFileSync(packagePath, withPackageVersion(readFileSync(packagePath, "utf8"), version));
  await Bun.$`cargo update --workspace --offline`.cwd(root).quiet().nothrow();
  await Bun.$`bunx git-cliff --tag v${version} -o CHANGELOG.md`.cwd(root).quiet();
  console.log(`version set to ${version}, CHANGELOG.md updated`);
}

if (import.meta.main) {
  const [command, arg] = process.argv.slice(2);
  if (command === "check") check(arg);
  else if (command === "set") await set(arg);
  else {
    console.error("usage: bun scripts/version.ts <check [tag] | set <version>>");
    process.exit(1);
  }
}
