// A minimal line-level diff (LCS) for the per-file diff viewer. Given the old
// and new text of a file, it returns a unified sequence of unchanged, added
// and removed lines — enough to render a GitHub-style patch.

export type LineOp = { type: "same" | "add" | "del"; text: string };

// Guard against the O(m·n) table blowing up on very large files; above this the
// viewer just shows the file as fully replaced.
const MAX_LINES = 4000;

export function lineDiff(oldText: string, newText: string): LineOp[] {
  const a = oldText === "" ? [] : oldText.replace(/\r\n/g, "\n").split("\n");
  const b = newText === "" ? [] : newText.replace(/\r\n/g, "\n").split("\n");

  if (a.length > MAX_LINES || b.length > MAX_LINES) {
    return [
      ...a.map((text): LineOp => ({ type: "del", text })),
      ...b.map((text): LineOp => ({ type: "add", text })),
    ];
  }

  const m = a.length;
  const n = b.length;
  // dp[i][j] = length of the LCS of a[i..] and b[j..].
  const dp: number[][] = Array.from({ length: m + 1 }, () => new Array<number>(n + 1).fill(0));
  for (let i = m - 1; i >= 0; i--) {
    for (let j = n - 1; j >= 0; j--) {
      dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  const ops: LineOp[] = [];
  let i = 0;
  let j = 0;
  while (i < m && j < n) {
    if (a[i] === b[j]) {
      ops.push({ type: "same", text: a[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      ops.push({ type: "del", text: a[i] });
      i++;
    } else {
      ops.push({ type: "add", text: b[j] });
      j++;
    }
  }
  while (i < m) ops.push({ type: "del", text: a[i++] });
  while (j < n) ops.push({ type: "add", text: b[j++] });
  return ops;
}
