import { describe, expect, it } from "vitest";
import { renderMarkdown, toggleTaskInMarkdown } from "./markdown";

describe("renderMarkdown — safety", () => {
  it("escapes raw HTML so scripts never reach the DOM", () => {
    const html = renderMarkdown('<script>alert("x")</script>');
    expect(html).not.toContain("<script>");
    expect(html).toContain("&lt;script&gt;");
  });

  it("escapes HTML inside inline code", () => {
    const html = renderMarkdown("`<img src=x onerror=alert(1)>`");
    expect(html).toContain("<code>&lt;img");
    expect(html).not.toContain("<img");
  });

  it("blocks javascript: links (renders the label only)", () => {
    const html = renderMarkdown("[click](javascript:alert(1))");
    expect(html).not.toContain("javascript:");
    expect(html).not.toContain("<a ");
    expect(html).toContain("click");
  });

  it("allows http(s) links with safe attributes", () => {
    const html = renderMarkdown("[site](https://example.com)");
    expect(html).toContain('href="https://example.com"');
    expect(html).toContain('rel="noreferrer noopener"');
  });

  it("does not treat underscores inside a URL as italics", () => {
    const html = renderMarkdown("[x](https://ex.com/a_b_c)");
    expect(html).toContain("https://ex.com/a_b_c");
    expect(html).not.toContain("<em>");
  });
});

describe("renderMarkdown — formatting", () => {
  it("renders headings", () => {
    expect(renderMarkdown("# Title")).toContain("<h1>Title</h1>");
    expect(renderMarkdown("### Sub")).toContain("<h3>Sub</h3>");
  });

  it("renders bold and italic", () => {
    expect(renderMarkdown("**bold**")).toContain("<strong>bold</strong>");
    expect(renderMarkdown("*it*")).toContain("<em>it</em>");
  });

  it("keeps snake_case words intact", () => {
    const html = renderMarkdown("call some_function_name here");
    expect(html).not.toContain("<em>");
    expect(html).toContain("some_function_name");
  });

  it("renders fenced code blocks", () => {
    const html = renderMarkdown("```\nline1\nline2\n```");
    expect(html).toContain("<pre><code>line1\nline2</code></pre>");
  });

  it("renders unordered and ordered lists", () => {
    expect(renderMarkdown("- a\n- b")).toContain("<ul><li>a</li><li>b</li></ul>");
    expect(renderMarkdown("1. a\n2. b")).toContain("<ol><li>a</li><li>b</li></ol>");
  });

  it("renders blockquotes and horizontal rules", () => {
    expect(renderMarkdown("> quoted")).toContain("<blockquote><p>quoted</p></blockquote>");
    expect(renderMarkdown("---")).toContain("<hr />");
  });
});

describe("renderMarkdown — task lists", () => {
  it("renders checkboxes with sequential data-task indices", () => {
    const html = renderMarkdown("- [ ] todo\n- [x] done");
    expect(html).toContain('class="task-list"');
    expect(html).toContain('data-task="0"');
    expect(html).toContain('data-task="1" checked');
    expect(html).toContain("<span>todo</span>");
    expect(html).toContain("<span>done</span>");
  });
});

describe("toggleTaskInMarkdown", () => {
  const src = "- [ ] first\n- [x] second\n- [ ] third";

  it("checks the requested task", () => {
    expect(toggleTaskInMarkdown(src, 0, true)).toBe("- [x] first\n- [x] second\n- [ ] third");
  });

  it("unchecks the requested task", () => {
    expect(toggleTaskInMarkdown(src, 1, false)).toBe("- [ ] first\n- [ ] second\n- [ ] third");
  });

  it("leaves other tasks untouched", () => {
    expect(toggleTaskInMarkdown(src, 2, true)).toBe("- [ ] first\n- [x] second\n- [x] third");
  });

  it("indices line up with the rendered data-task order", () => {
    const toggled = toggleTaskInMarkdown(src, 2, true);
    expect(renderMarkdown(toggled)).toContain('data-task="2" checked');
  });
});
