// Diff two content manifests (path → hash). Mirrors
// feather_core::snapshot::diff_manifests so commit-to-commit and deploy-to-
// deploy diffs can be computed in the UI from stored manifests, without
// downloading any snapshot.

import type { Diff, FileChange, Manifest } from "./types";

export function diffManifests(base: Manifest, next: Manifest): Diff {
  const changes: FileChange[] = [];
  for (const [path, hash] of Object.entries(next)) {
    if (!(path in base)) changes.push({ path, change: "added" });
    else if (base[path] !== hash) changes.push({ path, change: "modified" });
  }
  for (const path of Object.keys(base)) {
    if (!(path in next)) changes.push({ path, change: "deleted" });
  }
  changes.sort((a, b) => a.path.localeCompare(b.path));
  return { changes };
}

/** `(added, modified, deleted)` counts for a diff. */
export function diffCounts(diff: Diff): { added: number; modified: number; deleted: number } {
  let added = 0;
  let modified = 0;
  let deleted = 0;
  for (const c of diff.changes) {
    if (c.change === "added") added++;
    else if (c.change === "modified") modified++;
    else deleted++;
  }
  return { added, modified, deleted };
}
