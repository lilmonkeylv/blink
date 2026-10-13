import type { CommitSummary, DiffHunk, DiffLine, FileDiff, GraphEdge, GraphPage, GraphRow, LineKind } from "../api";

const VERSION = 1;
const KIND_DIFF = 1;
const KIND_HISTORY = 2;
const KIND_GRAPH = 3;
const LINE_KINDS: LineKind[] = ["context", "added", "removed"];
const utf8 = new TextDecoder();

export type WirePayload = ArrayBuffer | ArrayBufferView | number[];

function bytesOf(payload: WirePayload): Uint8Array {
  if (payload instanceof ArrayBuffer) return new Uint8Array(payload);
  if (ArrayBuffer.isView(payload)) return new Uint8Array(payload.buffer, payload.byteOffset, payload.byteLength);
  return Uint8Array.from(payload);
}

class Reader {
  private view: DataView;
  private pos = 6;
  private strings: string;
  private cursor = 0;

  constructor(payload: WirePayload, kind: number) {
    const bytes = bytesOf(payload);
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (bytes[0] !== VERSION || bytes[1] !== kind) throw new Error(`unexpected payload ${bytes[0]}/${bytes[1]}`);
    const head = this.view.getUint32(2, true);
    this.strings = utf8.decode(bytes.subarray(6 + head));
  }

  u8() {
    return this.view.getUint8(this.pos++);
  }

  u16() {
    const v = this.view.getUint16(this.pos, true);
    this.pos += 2;
    return v;
  }

  u32() {
    const v = this.view.getUint32(this.pos, true);
    this.pos += 4;
    return v;
  }

  f64() {
    const v = this.view.getFloat64(this.pos, true);
    this.pos += 8;
    return v;
  }

  str() {
    const end = this.cursor + this.u32();
    const s = this.strings.slice(this.cursor, end);
    this.cursor = end;
    return s;
  }

  commits(): CommitSummary[] {
    const commits = new Array<CommitSummary>(this.u32());
    for (let i = 0; i < commits.length; i++) {
      const id = this.str();
      const shortId = this.str();
      const summary = this.str();
      const authorName = this.str();
      const authorEmail = this.str();
      const time = this.f64();
      const parents = new Array<string>(this.u16());
      for (let p = 0; p < parents.length; p++) parents[p] = this.str();
      commits[i] = { id, shortId, summary, authorName, authorEmail, time, parents };
    }
    return commits;
  }

  edges(): GraphEdge[] {
    const edges = new Array<GraphEdge>(this.u16());
    for (let i = 0; i < edges.length; i++) edges[i] = { from: this.u16(), to: this.u16(), color: this.u16() };
    return edges;
  }
}

export function decodeDiff(payload: WirePayload): FileDiff {
  const r = new Reader(payload, KIND_DIFF);
  const flags = r.u8();
  const diff: FileDiff = { binary: !!(flags & 1), tooLarge: !!(flags & 2), additions: r.u32(), deletions: r.u32(), hunks: [] };
  const extra = r.str();
  if (extra) {
    const { submodule, lfs } = JSON.parse(extra);
    if (submodule) diff.submodule = submodule;
    if (lfs) diff.lfs = lfs;
  }
  diff.hunks = new Array<DiffHunk>(r.u32());
  for (let h = 0; h < diff.hunks.length; h++) {
    const hunk: DiffHunk = { oldStart: r.u32(), oldLen: r.u32(), newStart: r.u32(), newLen: r.u32(), lines: [] };
    hunk.lines = new Array<DiffLine>(r.u32());
    for (let i = 0; i < hunk.lines.length; i++) {
      const bits = r.u8();
      const oldNo = r.u32();
      const newNo = r.u32();
      const line: DiffLine = {
        kind: LINE_KINDS[bits & 3],
        oldNo: oldNo || null,
        newNo: newNo || null,
        text: r.str(),
        noNewline: !!(bits & 4),
      };
      if (bits & 8) {
        const tokens = new Array<[number, number, number]>(r.u32());
        for (let t = 0; t < tokens.length; t++) tokens[t] = [r.u32(), r.u32(), r.u8()];
        line.tokens = tokens;
      }
      hunk.lines[i] = line;
    }
    diff.hunks[h] = hunk;
  }
  return diff;
}

export function decodeHistory(payload: WirePayload): CommitSummary[] {
  return new Reader(payload, KIND_HISTORY).commits();
}

export function decodeGraph(payload: WirePayload): GraphPage {
  const r = new Reader(payload, KIND_GRAPH);
  const commits = r.commits();
  const rows = new Array<GraphRow>(r.u32());
  for (let i = 0; i < rows.length; i++) {
    const lane = r.u16();
    const color = r.u16();
    const width = r.u16();
    rows[i] = { lane, color, top: r.edges(), bottom: r.edges(), width };
  }
  return { commits, rows };
}
