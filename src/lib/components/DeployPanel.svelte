<script lang="ts">
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    checkRemoteDeploy,
    deployBundle,
    onDeployEvent,
    projectHistory,
    projectManifest,
    pullProject,
    rollbackToSnapshot,
  } from "../api";
  import {
    anonKey,
    currentBundle,
    getCommitManifest,
    listCommits,
    listDeploys,
    recordDeploy,
    releaseBundle,
    serverManifest,
    sessionToken,
    setServerManifest,
    STORAGE_ENDPOINT,
    type CloudProject,
    type DeployEntry,
    type DeployKind,
  } from "../cloud";
  import type { DeployStep, Manifest, ProjectConfig } from "../types";
  import CloudCommits from "./CloudCommits.svelte";
  import ProjectHistory from "./ProjectHistory.svelte";

  let {
    project,
    localPath,
    autoImport = false,
    onImported,
  }: {
    project: CloudProject;
    localPath: string | null;
    autoImport?: boolean;
    onImported?: () => void;
  } = $props();

  const config = $derived<ProjectConfig | null>(
    localPath
      ? {
          id: project.id,
          name: project.name,
          local_path: localPath,
          panel_id: project.panel_id ?? "",
          server_identifier: project.server_identifier ?? "",
          target_dir: project.target_dir,
          build_command: project.build_command,
          post_deploy: project.post_deploy,
          auto_backup: project.auto_backup,
        }
      : null,
  );

  let step = $state<DeployStep | null>(null);
  let showHistory = $state(false);
  // When the history drawer is opened from a Deploy-history row, the row's
  // timestamp so the drawer can jump straight to that deploy.
  let focusDeployAt = $state<string | null>(null);
  let error = $state<string | null>(null);
  let backupWarning = $state<string | null>(null);
  let deploys = $state<DeployEntry[]>([]);
  // Bumped after a deploy so the cloud-commits panel reloads (server state
  // changed → diff and the current Deploy reset).
  let cloudRefresh = $state(0);
  // Whether an in-flight engine run should be recorded, and as what.
  let currentKind: DeployKind | null = null;
  // The in-flight bundle deploy's summary + the server state it produces (the
  // newest commit's manifest), used to release the bundle once it lands.
  let pendingDeploy: { summary: string | null; manifest: Manifest } | null = null;
  // An import is running; on completion the local folder mirrors the server, so
  // we record it as the diff baseline.
  let pullPending = false;
  // A teammate deployed a newer version but our working tree is dirty, so we
  // can't auto-sync — shown as a banner until the tree is clean.
  let syncBlocked = $state(false);

  const running = $derived(step !== null && step.step !== "done" && step.step !== "failed");

  const progressLabel = $derived.by(() => {
    if (!step) return null;
    switch (step.step) {
      case "committing": return "Committing…";
      case "checking_out": return "Checking out…";
      case "building": return "Building…";
      case "backing_up": return "Backing up…";
      case "scanning": return "Scanning…";
      case "packing": return `Packing ${step.files} files…`;
      case "uploading": return `Uploading ${step.percent}%`;
      case "downloading": return `Downloading ${step.percent}%`;
      case "importing": return "Importing files…";
      case "extracting": return "Extracting…";
      case "cleaning_up": return "Cleaning up…";
      case "restarting": return "Restarting…";
      default: return null;
    }
  });

  let unlisten: UnlistenFn | undefined;
  let syncTimer: ReturnType<typeof setInterval> | undefined;
  onMount(() => {
    void loadDeploys();
    onDeployEvent(project.id, handleStep).then((u) => {
      unlisten = u;
      // Auto-import the server's files right after linking an empty folder, so
      // the diff is meaningful immediately. Listen first, then start.
      if (autoImport && config) {
        onImported?.();
        void importFiles();
      } else {
        // Only check now when nothing else is starting an engine run, so the
        // sync pull can't race the auto-import for the engine slot.
        void checkSync();
      }
      // Keep this device in sync with teammates' deploys: poll the server's
      // deploy marker and pull the new state into the local folder when clean.
      syncTimer = setInterval(() => void checkSync(), 30_000);
    });
    return () => {
      unlisten?.();
      if (syncTimer) clearInterval(syncTimer);
    };
  });

  async function loadDeploys() {
    try {
      deploys = await listDeploys(project.id);
    } catch {
      // timeline is best effort
    }
  }

  /**
   * Keep the local folder current with teammates' deploys. If the server
   * announces a deploy newer than this device's record, pull it in — but only
   * when the working tree is clean, so uncommitted work is never overwritten
   * (a dirty tree shows a banner instead).
   */
  async function checkSync() {
    const cfg = config;
    if (!cfg || running) return;
    try {
      const info = await checkRemoteDeploy(cfg);
      if (!info.newer) {
        syncBlocked = false;
        return;
      }
      if (info.dirty) {
        syncBlocked = true;
        return;
      }
      syncBlocked = false;
      currentKind = null; // a sync is not recorded as a deploy…
      pullPending = true; // …but it does make the local folder mirror the server
      step = { step: "downloading", percent: 0 };
      await pullProject(cfg, "sync");
    } catch {
      // best effort — never disrupt the tab
    }
  }

  function handleStep(s: DeployStep) {
    // Build output is log-style; ignore it here.
    if (s.step === "build_output") return;
    // A skipped backup isn't fatal, but the user must see it — keep it visible
    // through the rest of the deploy.
    if (s.step === "backup_skipped") {
      backupWarning = s.reason;
      return;
    }
    step = s;
    if (s.step === "done" || s.step === "failed") {
      const kind = currentKind;
      currentKind = null;
      if (kind) void record(kind, s);
      // After importing/syncing the server's files, the local folder mirrors
      // the server — record that as the diff baseline so the Deploy tab doesn't
      // show every file as changed, and refresh the shared timeline.
      if (s.step === "done" && pullPending) {
        void setImportBaseline();
        void loadDeploys();
      }
      pullPending = false;
    }
  }

  /** After a successful import, the local manifest == the server state. */
  async function setImportBaseline() {
    if (!config) return;
    try {
      const manifest = await projectManifest(config);
      await setServerManifest(project.id, manifest);
      cloudRefresh += 1;
    } catch (e) {
      console.error("could not set server baseline:", e);
    }
  }

  async function record(kind: DeployKind, s: DeployStep) {
    try {
      if (s.step === "done") {
        let commit: string | null = null;
        let summary: string | null = null;
        if (kind === "deploy" && pendingDeploy) {
          // A bundle deploy ships cloud commits, not a local git commit — label
          // it with the newest commit's message.
          summary = pendingDeploy.summary;
        } else {
          try {
            const [head] = await projectHistory(config!, 1);
            if (head) {
              commit = head.short_id;
              summary = head.summary;
            }
          } catch {
            // history is a nicety
          }
        }
        await recordDeploy({
          projectId: project.id,
          kind,
          status: "success",
          commit,
          commitSummary: summary,
          files: s.files,
        });
        // A successful deploy shipped the current bundle: mark it released,
        // recording the new server state (the newest commit's manifest) so the
        // diff resets and a fresh Deploy opens. Best effort — never fails the run.
        if (kind === "deploy" && pendingDeploy) {
          await releaseCurrentBundle(pendingDeploy.manifest);
        }
      } else if (s.step === "failed") {
        await recordDeploy({ projectId: project.id, kind, status: "failed", message: s.message });
      }
      pendingDeploy = null;
      await loadDeploys();
    } catch (e) {
      console.error("could not record deploy:", e);
    }
  }

  /**
   * Mark the project's current Deploy bundle released, recording `manifest` as
   * the state now on the server so future "local vs server" diffs are correct
   * and a fresh Deploy opens for the next commits. Best effort: the files are
   * already live, so bundle bookkeeping must never surface as a deploy failure.
   */
  async function releaseCurrentBundle(manifest: Manifest) {
    try {
      await currentBundle(project.id); // ensure a pending bundle exists to release
      await releaseBundle(project.id, Object.keys(manifest).length, null, manifest);
      cloudRefresh += 1;
    } catch (e) {
      console.error("bundle release skipped:", e);
    }
  }

  async function deploy() {
    if (!config) return;
    error = null;
    backupWarning = null;
    try {
      // A deploy ships only committed work: gather the current bundle's stored
      // commits (oldest first) and apply their deltas over the server state.
      const base = await serverManifest(project.id);
      const b = await currentBundle(project.id);
      const stored = (await listCommits(project.id, b.id)).filter((c) => c.stored).reverse();
      if (stored.length === 0) {
        error = "Nothing committed to deploy yet — commit your changes first.";
        return;
      }
      const commits = await Promise.all(
        stored.map(async (c) => ({ id: c.id, manifest: await getCommitManifest(c.id) })),
      );
      const newest = commits[commits.length - 1];
      pendingDeploy = { summary: stored[stored.length - 1].message, manifest: newest.manifest };
      currentKind = "deploy";
      step = { step: "downloading", percent: 0 };
      const token = await sessionToken();
      await deployBundle(config, STORAGE_ENDPOINT, token, anonKey, project.id, base, commits);
    } catch (e) {
      const failed: DeployStep = { step: "failed", message: String(e) };
      step = failed;
      currentKind = null;
      pendingDeploy = null;
      void record("deploy", failed);
    }
  }

  async function importFiles() {
    if (!config) return;
    error = null;
    currentKind = null; // imports are not recorded as deploys
    pullPending = true; // …but they do reset the diff baseline
    step = { step: "downloading", percent: 0 };
    try {
      await pullProject(config, "import");
    } catch (e) {
      step = { step: "failed", message: String(e) };
      pullPending = false;
    }
  }

  async function onRollback(commitId: string) {
    if (!config) return;
    showHistory = false;
    error = null;
    backupWarning = null;
    currentKind = "rollback";
    step = { step: "downloading", percent: 0 };
    try {
      const token = await sessionToken();
      await rollbackToSnapshot(config, STORAGE_ENDPOINT, token, anonKey, project.id, commitId);
    } catch (e) {
      const failed: DeployStep = { step: "failed", message: String(e) };
      step = failed;
      currentKind = null;
      void record("rollback", failed);
    }
  }

  /** Open the shared history drawer focused on the clicked deploy. */
  function openDeployDetail(d: DeployEntry) {
    focusDeployAt = d.created_at;
    showHistory = true;
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }
</script>

<div class="deploy">
  {#if !config}
    <div class="card notice">
      <p>
        No local folder on this device. Add one under <strong>Settings → Local
        folder</strong> to deploy and commit from here. You can still see the
        shared deploy history below.
      </p>
    </div>
  {:else}
    {#if syncBlocked}
      <div class="card sync-banner">
        <p>
          ⬇ A teammate shipped a newer deploy. Commit or discard your local
          changes and it will sync into this folder automatically.
        </p>
      </div>
    {/if}

    <CloudCommits {project} {config} refresh={cloudRefresh} onCommitted={loadDeploys} />

    <div class="card actions-card">
      <div class="deploy-row">
        <button class="primary" onclick={deploy} disabled={running}>
          {running ? "Working…" : "Deploy"}
        </button>
        <button class="ghost" onclick={importFiles} disabled={running} title="Pull the server's files into the local folder">
          Import from server
        </button>
        <button class="ghost" onclick={() => (showHistory = true)} disabled={running}>
          History
        </button>
      </div>

      {#if running && progressLabel}
        <div class="progress">
          <span>{progressLabel}</span>
          <div class="bar">
            <div
              class="fill"
              class:indeterminate={!(step?.step === "uploading" || step?.step === "downloading")}
              style="width: {step?.step === 'uploading' || step?.step === 'downloading' ? step.percent : 100}%"
            ></div>
          </div>
        </div>
      {:else if step?.step === "failed"}
        <p class="error" title={step.message}>Failed: {step.message}</p>
      {:else if step?.step === "done"}
        <p class="ok">Deployed ✓ {step.files} files{step.deleted > 0 ? `, ${step.deleted} removed` : ""}</p>
      {/if}

      {#if error}<p class="error">{error}</p>{/if}
      {#if backupWarning}
        <p class="warn" title={backupWarning}>⚠ No backup was made — {backupWarning}</p>
      {/if}
    </div>
  {/if}

  <h3 class="timeline-title">Deploy history</h3>
  {#if deploys.length === 0}
    <p class="muted center empty">No deploys recorded yet.</p>
  {:else}
    <ul class="timeline">
      {#each deploys as d (d.id)}
        <li>
          <button class="d-row" onclick={() => openDeployDetail(d)} title="Open this deploy's history">
            <span class="badge {d.status}">{d.status === "success" ? "✓" : "✕"}</span>
            <div class="d-main">
              <span class="d-title">
                <span class="d-kind">{d.kind}</span>
                {#if d.commit_summary}<span class="d-summary">{d.commit_summary}</span>
                {:else if d.status === "failed" && d.message}<span class="d-summary fail">{d.message}</span>{/if}
              </span>
              <span class="d-meta muted">
                {#if d.commit}<span class="mono">{d.commit}</span> · {/if}
                {#if d.files_count !== null}{d.files_count} files · {/if}
                {d.display_name?.trim() || d.username || "someone"} · {when(d.created_at)}
              </span>
            </div>
            <span class="chev" aria-hidden="true">›</span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if showHistory}
  <ProjectHistory
    {project}
    {onRollback}
    {focusDeployAt}
    onClose={() => {
      showHistory = false;
      focusDeployAt = null;
    }}
  />
{/if}

<style>
  .card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 16px;
    margin-bottom: 22px;
  }

  .sync-banner {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 10%, var(--surface));
  }

  .sync-banner p {
    margin: 0;
    line-height: 1.5;
    font-size: 13px;
  }

  .notice p {
    margin: 0;
    line-height: 1.5;
  }

  .deploy-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .progress {
    margin-top: 12px;
    font-size: 12px;
    color: var(--accent);
  }

  .bar {
    height: 5px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
    margin-top: 4px;
  }

  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.5s ease;
  }

  .fill.indeterminate {
    animation: pulse 1.2s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 0.35; }
    50% { opacity: 1; }
  }

  .timeline-title {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    margin-bottom: 12px;
  }

  .empty {
    padding: 24px 0;
  }

  .timeline {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .d-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    text-align: left;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
  }

  .d-row:hover {
    border-color: var(--accent);
  }

  .chev {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 18px;
    color: var(--text-muted);
  }

  .d-row:hover .chev {
    color: var(--accent);
  }

  .badge {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 12px;
    font-weight: 700;
  }

  .badge.success {
    background: #10b98122;
    color: #34d399;
  }

  .badge.failed {
    background: #ef444422;
    color: #f87171;
  }

  .d-main {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .d-title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  .d-kind {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
  }

  .d-summary {
    font-weight: 600;
    font-size: 14px;
  }

  .d-summary.fail {
    font-weight: 400;
    color: var(--text-muted);
  }

  .d-meta {
    font-size: 12px;
  }

  .mono {
    font-family: ui-monospace, monospace;
  }
</style>
