import type { FileDiff, FileSelection } from "../api";

export type CheckState = "all" | "none" | "partial";

export interface FileSel {
  included: boolean;
  excluded: ReadonlySet<string>;
  signature?: string;
}

export const lineKey = (hunk: number, line: number) => `${hunk}:${line}`;

export function changedLineKeys(diff: FileDiff): string[] {
  const keys: string[] = [];
  diff.hunks.forEach((h, hi) =>
    h.lines.forEach((l, li) => {
      if (l.kind !== "context") keys.push(lineKey(hi, li));
    }),
  );
  return keys;
}

export function diffSignature(diff: FileDiff): string {
  return diff.hunks.map((h) => `${h.oldStart},${h.oldLen},${h.newStart},${h.newLen}`).join("|") + `#${diff.additions}/${diff.deletions}`;
}

export const defaultSel = (): FileSel => ({ included: true, excluded: new Set() });

export function checkState(sel: FileSel | undefined, diff?: FileDiff): CheckState {
  const s = sel ?? defaultSel();
  if (!s.included) return "none";
  if (s.excluded.size === 0) return "all";
  if (diff && changedLineKeys(diff).every((k) => s.excluded.has(k))) return "none";
  return "partial";
}

export function isLineIncluded(sel: FileSel | undefined, key: string): boolean {
  const s = sel ?? defaultSel();
  return s.included && !s.excluded.has(key);
}

export function toggleFile(sel: FileSel | undefined, diff?: FileDiff): FileSel {
  const state = checkState(sel, diff);
  return { included: state !== "all", excluded: new Set(), signature: sel?.signature };
}

export function setLines(sel: FileSel | undefined, keys: string[], include: boolean, diff: FileDiff): FileSel {
  const s = sel ?? defaultSel();
  const excluded = new Set(s.included ? s.excluded : changedLineKeys(diff));
  for (const k of keys) {
    if (include) excluded.delete(k);
    else excluded.add(k);
  }
  const all = changedLineKeys(diff);
  const included = !all.every((k) => excluded.has(k));
  return { included, excluded: included ? excluded : new Set(), signature: diffSignature(diff) };
}

export function reconcile(sel: FileSel | undefined, diff: FileDiff): FileSel | undefined {
  if (!sel || sel.excluded.size === 0) return sel;
  if (sel.signature && sel.signature !== diffSignature(diff)) {
    return { included: sel.included, excluded: new Set() };
  }
  return sel;
}

export function toPayload(path: string, sel: FileSel | undefined, diff?: FileDiff): FileSelection | null {
  const s = sel ?? defaultSel();
  if (!s.included) return null;
  if (s.excluded.size === 0 || !diff) return { path };
  const lines: [number, number][] = changedLineKeys(diff)
    .filter((k) => !s.excluded.has(k))
    .map((k) => k.split(":").map(Number) as [number, number]);
  return lines.length ? { path, lines } : null;
}
