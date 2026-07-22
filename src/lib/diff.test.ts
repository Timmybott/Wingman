import { describe, expect, it } from "vitest";
import { diffCounts, diffManifests } from "./diff";

describe("diffManifests", () => {
  it("detects added, modified and deleted paths", () => {
    const base = { "keep.txt": "h1", "change.txt": "h2", "gone.txt": "h3" };
    const next = { "keep.txt": "h1", "change.txt": "h2-new", "new.txt": "h4" };
    const diff = diffManifests(base, next);
    expect(diffCounts(diff)).toEqual({ added: 1, modified: 1, deleted: 1 });
    expect(diff.changes).toContainEqual({ path: "new.txt", change: "added" });
    expect(diff.changes).toContainEqual({ path: "change.txt", change: "modified" });
    expect(diff.changes).toContainEqual({ path: "gone.txt", change: "deleted" });
  });

  it("is empty for identical manifests", () => {
    const m = { "a.txt": "x", "b.txt": "y" };
    expect(diffManifests(m, m).changes).toEqual([]);
  });

  it("sorts changes by path", () => {
    const diff = diffManifests({}, { "z.txt": "1", "a.txt": "2", "m.txt": "3" });
    expect(diff.changes.map((c) => c.path)).toEqual(["a.txt", "m.txt", "z.txt"]);
  });
});
