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
