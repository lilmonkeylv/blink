import type { Submodule } from "../api";

export function missingSubmodules(list: Submodule[]): Submodule[] {
  return list.filter((s) => !s.initialized);
}

export function missingSubmodulesMessage(missing: Submodule[]): string {
  if (missing.length === 1) return `Submodule ${missing[0].path} isn't checked out yet.`;
  return `${missing.length} submodules aren't checked out yet.`;
}
