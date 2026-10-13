export type LogLineKind = "plain" | "group" | "error" | "warning" | "command";

export interface LogLine {
  text: string;
  kind: LogLineKind;
}

export interface JobLog {
  lines: LogLine[];
  hidden: number;
  firstError: number;
}

const TIMESTAMP = /^\uFEFF?\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z ?/;
const MARKER = /^##\[(group|endgroup|error|warning|command)\]/;

function classify(raw: string): LogLine | null {
  const marker = MARKER.exec(raw);
  if (!marker) return { text: raw, kind: "plain" };
  if (marker[1] === "endgroup") return null;
  return { text: raw.slice(marker[0].length), kind: marker[1] as LogLineKind };
}

export function parseJobLog(source: string, limit = 5000): JobLog {
  const all = source.replace(/\r?\n$/, "").split(/\r?\n/);
  const hidden = Math.max(0, all.length - limit);
  const lines = all
    .slice(hidden)
    .map((line) => classify(line.replace(TIMESTAMP, "")))
    .filter((line): line is LogLine => line !== null);
  let firstError = lines.findIndex((l) => l.kind === "error");
  if (firstError < 0) firstError = lines.findIndex((l) => /\berror\b/i.test(l.text));
  return { lines, hidden, firstError };
}
