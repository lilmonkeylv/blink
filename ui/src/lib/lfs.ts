export const LFS_INSTALL_URL = "https://git-lfs.com";
const WARNED_KEY = "blink.lfsMissingWarned";

export function lfsPatternFor(path: string): string {
  const name = path.split("/").pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 && dot < name.length - 1 ? `*${name.slice(dot)}` : path;
}

type Storage = Pick<globalThis.Storage, "getItem" | "setItem">;

function warned(storage: Storage): string[] {
  try {
    const parsed = JSON.parse(storage.getItem(WARNED_KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((r): r is string => typeof r === "string") : [];
  } catch {
    return [];
  }
}

export function claimMissingLfsWarning(repo: string, storage: Storage): boolean {
  const seen = warned(storage);
  if (seen.includes(repo)) return false;
  try {
    storage.setItem(WARNED_KEY, JSON.stringify([...seen, repo].slice(-200)));
  } catch {}
  return true;
}
