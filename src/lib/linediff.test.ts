import { describe, expect, it } from "vitest";
import { lineDiff } from "./linediff";

describe("lineDiff", () => {
  it("marks unchanged lines as same", () => {
    const ops = lineDiff("a\nb\nc", "a\nb\nc");
    expect(ops.every((o) => o.type === "same")).toBe(true);
    expect(ops.map((o) => o.text)).toEqual(["a", "b", "c"]);
  });

  it("detects an added line", () => {
    const ops = lineDiff("a\nc", "a\nb\nc");
    expect(ops).toEqual([
      { type: "same", text: "a" },
      { type: "add", text: "b" },
      { type: "same", text: "c" },
    ]);
  });

  it("detects a removed line", () => {
    const ops = lineDiff("a\nb\nc", "a\nc");
    expect(ops).toEqual([
      { type: "same", text: "a" },
      { type: "del", text: "b" },
      { type: "same", text: "c" },
    ]);
  });

  it("detects a changed line as del + add", () => {
    const ops = lineDiff("hello", "world");
    expect(ops).toContainEqual({ type: "del", text: "hello" });
    expect(ops).toContainEqual({ type: "add", text: "world" });
  });

  it("treats an empty old file as all additions", () => {
    const ops = lineDiff("", "x\ny");
    expect(ops).toEqual([
      { type: "add", text: "x" },
      { type: "add", text: "y" },
    ]);
  });

  it("normalizes CRLF line endings", () => {
    const ops = lineDiff("a\r\nb", "a\nb");
    expect(ops.every((o) => o.type === "same")).toBe(true);
  });
});
