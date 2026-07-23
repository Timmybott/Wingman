// Reconstruct a file's content from delta snapshots (M35).
//
// A commit stores only its delta, so a file's content "as of" a commit is not
// in that one zip unless the commit changed it. To show a correct diff we walk
// from a starting commit toward older commits and take the content from the
// first delta that actually wrote the path. Commits are passed newest-first.

import { snapshotFile } from "./api";
import { anonKey, STORAGE_ENDPOINT } from "./cloud";

/**
 * The content of `path` as of the accumulated state at `commits[startIdx]`,
 * found by walking older commits until the delta that wrote it. Returns
 * `found: false` when no commit in the list wrote the path — it was inherited
 * from the server (whose bytes aren't stored), so the caller may fall back to
 * the live server file.
 */
export async function fileContentAt(
  commits: { id: string }[],
  startIdx: number,
  path: string,
  token: string,
  projectId: string,
): Promise<{ found: boolean; text: string }> {
  for (let j = startIdx; j < commits.length; j++) {
    const r = await snapshotFile(STORAGE_ENDPOINT, token, anonKey, projectId, commits[j].id, path);
    if (r.found) return r;
  }
  return { found: false, text: "" };
}
