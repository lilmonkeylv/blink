export type Inline =
  | { type: "text"; text: string }
  | { type: "code"; text: string }
  | { type: "strong"; children: Inline[] }
  | { type: "link"; href: string; children: Inline[] };

export type Block =
  | { type: "heading"; level: number; children: Inline[] }
  | { type: "list"; items: Inline[][] }
  | { type: "paragraph"; children: Inline[] };

const INLINE = /`([^`]+)`|\*\*(.+?)\*\*|\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/g;

export function parseInline(text: string): Inline[] {
  const out: Inline[] = [];
  let last = 0;
  for (const m of text.matchAll(INLINE)) {
    const index = m.index ?? 0;
    if (index > last) out.push({ type: "text", text: text.slice(last, index) });
    if (m[1] !== undefined) out.push({ type: "code", text: m[1] });
    else if (m[2] !== undefined) out.push({ type: "strong", children: parseInline(m[2]) });
    else out.push({ type: "link", href: m[4], children: parseInline(m[3]) });
    last = index + m[0].length;
  }
  if (last < text.length) out.push({ type: "text", text: text.slice(last) });
  return out;
}

export function parseMarkdown(source: string): Block[] {
  const blocks: Block[] = [];
  let paragraph: string[] = [];
  let list: Inline[][] | null = null;

  const flush = () => {
    if (paragraph.length) blocks.push({ type: "paragraph", children: parseInline(paragraph.join(" ")) });
    paragraph = [];
    if (list) blocks.push({ type: "list", items: list });
    list = null;
  };

  for (const raw of source.split("\n")) {
    const line = raw.trimEnd();
    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    const item = line.match(/^\s*[-*]\s+(.*)$/);
    if (!line.trim()) {
      flush();
    } else if (heading) {
      flush();
      blocks.push({ type: "heading", level: heading[1].length, children: parseInline(heading[2]) });
    } else if (item) {
      if (paragraph.length) {
        blocks.push({ type: "paragraph", children: parseInline(paragraph.join(" ")) });
        paragraph = [];
      }
      list ??= [];
      list.push(parseInline(item[1]));
    } else if (list) {
      const items: Inline[][] = list;
      const lastItem = items[items.length - 1];
      lastItem.push({ type: "text", text: " " }, ...parseInline(line.trim()));
    } else {
      paragraph.push(line.trim());
    }
  }
  flush();
  return blocks;
}
