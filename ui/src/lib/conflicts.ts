export type Resolution = "ours" | "theirs" | "both" | "theirs-first" | null;

export interface Conflict {
  type: "conflict";
  ours: string[];
  base: string[] | null;
  theirs: string[];
  oursLabel: string;
  theirsLabel: string;
}

export type Segment = { type: "text"; lines: string[] } | Conflict;

export function parseConflicts(text: string): Segment[] {
  const lines = text.split("\n");
  const segments: Segment[] = [];
  let plain: string[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (line.startsWith("<<<<<<<")) {
      const start = i;
      const conflict: Conflict = { type: "conflict", ours: [], base: null, theirs: [], oursLabel: line.slice(7).trim(), theirsLabel: "" };
      let section: "ours" | "base" | "theirs" = "ours";
      i++;
      let closed = false;
      while (i < lines.length) {
        const l = lines[i];
        if (section === "ours" && l.startsWith("|||||||")) {
          section = "base";
          conflict.base = [];
        } else if (section !== "theirs" && l.startsWith("=======")) {
          section = "theirs";
        } else if (section === "theirs" && l.startsWith(">>>>>>>")) {
          conflict.theirsLabel = l.slice(7).trim();
          closed = true;
          break;
        } else if (section === "base") {
          conflict.base!.push(l);
        } else {
          conflict[section].push(l);
        }
        i++;
      }
      if (!closed) {
        plain.push(...lines.slice(start));
        break;
      }
      if (plain.length) segments.push({ type: "text", lines: plain });
      plain = [];
      segments.push(conflict);
      i++;
      continue;
    }
    plain.push(line);
    i++;
  }
  if (plain.length) segments.push({ type: "text", lines: plain });
  return segments;
}

export function conflictCount(segments: Segment[]): number {
  return segments.filter((s) => s.type === "conflict").length;
}

function resolved(c: Conflict, r: Resolution): string[] | null {
  switch (r) {
    case "ours":
      return c.ours;
    case "theirs":
      return c.theirs;
    case "both":
      return [...c.ours, ...c.theirs];
    case "theirs-first":
      return [...c.theirs, ...c.ours];
    default:
      return null;
  }
}

export function render(segments: Segment[], resolutions: Resolution[]): { text: string; unresolved: number } {
  const out: string[] = [];
  let index = 0;
  let unresolved = 0;
  for (const segment of segments) {
    if (segment.type === "text") {
      out.push(...segment.lines);
      continue;
    }
    const lines = resolved(segment, resolutions[index++] ?? null);
    if (lines) {
      out.push(...lines);
    } else {
      unresolved++;
      out.push(`<<<<<<< ${segment.oursLabel}`, ...segment.ours, "=======", ...segment.theirs, `>>>>>>> ${segment.theirsLabel}`);
    }
  }
  return { text: out.join("\n"), unresolved };
}

const ANCHOR = 3;
const MAX_SKIP = 2;

function occurrences(lines: string[], block: string[], from: number): number[] {
  const found: number[] = [];
  for (let i = from; i + block.length <= lines.length; i++) {
    if (block.every((l, k) => lines[i + k] === l)) found.push(i);
  }
  return found;
}

function anchors(lines: string[], fromEnd: boolean): string[][] {
  const out: string[][] = [];
  for (let skip = 0; skip <= Math.min(MAX_SKIP, lines.length - 1); skip++) {
    for (let size = Math.min(ANCHOR, lines.length - skip); size >= 1; size--) {
      const block = fromEnd ? lines.slice(lines.length - skip - size, lines.length - skip) : lines.slice(skip, skip + size);
      if (block.some((l) => l.trim())) out.push(block);
    }
  }
  return out;
}

const stripCr = (l: string) => (l.endsWith("\r") ? l.slice(0, -1) : l);

export function baseSections(segments: Segment[], baseText: string): (string[] | null)[] {
  const base = baseText.split("\n").map(stripCr);
  const sections: (string[] | null)[] = [];
  let cursor = 0;
  segments.forEach((segment, j) => {
    if (segment.type !== "conflict") return;
    if (segment.base) {
      sections.push(segment.base);
      return;
    }
    const prev = segments[j - 1];
    const next = segments[j + 1];
    let start: number | null = cursor;
    if (prev?.type === "text") {
      start = null;
      const lines = prev.lines.map(stripCr);
      for (const block of anchors(lines, true)) {
        const found = occurrences(base, block, cursor);
        if (!found.length) continue;
        const expected = cursor + lines.length - block.length;
        const closest = found.reduce((a, b) => (Math.abs(b - expected) < Math.abs(a - expected) ? b : a));
        start = closest + block.length;
        break;
      }
    }
    let end: number | null = base.length;
    if (start !== null && next?.type === "text") {
      const candidates = anchors(next.lines.map(stripCr), false);
      const trailing = candidates.length === 0 && j + 1 === segments.length - 1;
      end = trailing ? Math.max(start, base.length - next.lines.length) : null;
      for (const block of candidates) {
        const found = occurrences(base, block, start);
        if (found.length) {
          end = found[0];
          break;
        }
      }
    }
    if (start === null || end === null) {
      sections.push(null);
      return;
    }
    sections.push(base.slice(start, end));
    cursor = end;
  });
  return sections;
}
