import type { SigningStatus } from "../api";

export const DEFAULT_KEY_NAME = "id_ed25519_signing";

export function keyName(key: string): string {
  return key.split(/[\\/]/).pop() || key;
}

const SCOPES: Record<string, string> = {
  global: "your global Git config",
  local: "this repository's config",
  system: "the system Git config",
  worktree: "this worktree's config",
};

export function describeSigning(status: SigningStatus): string {
  if (!status.key) return "Commits aren't signed yet. Pick or create a key below.";
  const kind = status.format === "ssh" ? "SSH key" : "GPG key";
  const where = status.scope ? ` from ${SCOPES[status.scope] ?? `the ${status.scope} config`}` : "";
  const off = status.enabled ? "" : " Automatic signing (commit.gpgsign) is off.";
  return `Signing with ${kind} ${keyName(status.key)}${where}.${off}`;
}

export function matchingKey(status: SigningStatus | undefined, keys: { id: string }[]): string | null {
  if (!keys.length) return null;
  const current = status?.key;
  const same = current && keys.find((k) => k.id === current || keyName(k.id) === keyName(current) || k.id.toUpperCase().endsWith(current.toUpperCase()));
  return same ? same.id : keys[0].id;
}
