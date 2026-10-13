import { describe, expect, test } from "bun:test";
import { parseInline, parseMarkdown } from "./markdown";

describe("markdown", () => {
  test("headings, lists and paragraphs", () => {
    const blocks = parseMarkdown("### Features\n\n- one\n- two\n\nSome text\nwrapped.");
    expect(blocks.map((b) => b.type)).toEqual(["heading", "list", "paragraph"]);
    expect(blocks[1]).toMatchObject({ type: "list", items: [[{ text: "one" }], [{ text: "two" }]] });
    expect(blocks[2]).toMatchObject({ children: [{ type: "text", text: "Some text wrapped." }] });
  });

  test("inline code, bold and https links", () => {
    const inline = parseInline("**watch:** fix `blink.log` ([abc1234](https://github.com/lilmonkeylv/blink/commit/abc))");
    expect(inline.map((i) => i.type)).toEqual(["strong", "text", "code", "text", "link", "text"]);
    expect(inline[4]).toMatchObject({ href: "https://github.com/lilmonkeylv/blink/commit/abc" });
  });

  test("non-https links and html stay plain text", () => {
    const inline = parseInline("[x](javascript:alert(1)) <img src=x onerror=alert(1)>");
    expect(inline).toEqual([{ type: "text", text: "[x](javascript:alert(1)) <img src=x onerror=alert(1)>" }]);
  });
});
