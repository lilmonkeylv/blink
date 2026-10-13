import type { DiffHunk, DiffLine } from "../api";
import { lineKey } from "./selection";

export interface Cell {
  data: DiffLine;
  key: string;
}

export type SplitRow = { type: "hunk"; hunk: number; data: DiffHunk } | { type: "pair"; hunk: number; left?: Cell; right?: Cell };

export function pairLines(hunks: DiffHunk[]): SplitRow[] {
  const out: SplitRow[] = [];
  hunks.forEach((h, hi) => {
    out.push({ type: "hunk", hunk: hi, data: h });
    let removed: Cell[] = [];
    let added: Cell[] = [];
    const flush = () => {
      for (let i = 0; i < Math.max(removed.length, added.length); i++) {
        out.push({ type: "pair", hunk: hi, left: removed[i], right: added[i] });
      }
      removed = [];
      added = [];
    };
    h.lines.forEach((l, li) => {
      const cell = { data: l, key: lineKey(hi, li) };
      if (l.kind === "removed") {
        if (added.length) flush();
        removed.push(cell);
      } else if (l.kind === "added") {
        added.push(cell);
      } else {
        flush();
        out.push({ type: "pair", hunk: hi, left: cell, right: cell });
      }
    });
    flush();
  });
  return out;
}
