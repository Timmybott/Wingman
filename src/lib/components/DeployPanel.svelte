<script lang="ts">
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import {
    commitProject,
    deployProject,
    onDeployEvent,
    projectHistory,
    pullProject,
    repoStatus,
    rollbackProject,
  } from "../api";
  import {
    listDeploys,
    recordDeploy,
    type CloudProject,
    type DeployEntry,
    type DeployKind,
  } from "../cloud";
  import type { DeployStep, ProjectConfig, RepoStatus } from "../types";
  import HistoryView from "./HistoryView.svelte";

  let { project, localPath }: { project: CloudProject; localPath: string | null } = $props();

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
  let gitStatus = $state<RepoStatus | null>(null);
  let commitMessage = $state("");
  let committing = $state(false);
  let showHistory = $state(false);
  let error = $state<string | null>(null);
  let deploys = $state<DeployEntry[]>([]);
  // Whether an in-flight engine run should be recorded, and as what.
  let currentKind: DeployKind | null = null;

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
  onMount(() => {
    void loadDeploys();
    void refreshStatus();
    onDeployEvent(project.id, handleStep).then((u) => (unlisten = u));
    return () => unlisten?.();
  });

  async function loadDeploys() {
    try {
      deploys = await listDeploys(project.id);
    } catch {
      // timeline is best effort
    }
  }

  async function refreshStatus() {
    if (!config) {
      gitStatus = null;
      return;
    }
    try {
      gitStatus = await repoStatus(config);
    } catch {
      gitStatus = null;
    }
  }

  function handleStep(s: DeployStep) {
    // Log-style events don't change the tile state.
    if (s.step === "build_output" || s.step === "backup_skipped") return;
    step = s;
    if (s.step === "done" || s.step === "failed") {
      void refreshStatus();
      const kind = currentKind;
      currentKind = null;
      if (kind) void record(kind, s);
    }
  }

  async function record(kind: DeployKind, s: DeployStep) {
    try {
      if (s.step === "done") {
        let commit: string | null = null;
        let summary: string | null = null;
        try {
          const [head] = await projectHistory(config!, 1);
          if (head) {
            commit = head.short_id;
            summary = head.summary;
          }
        } catch {
          // history is a nicety
        }
        await recordDeploy({
          projectId: project.id,
          kind,
          status: "success",
          commit,
          commitSummary: summary,
          files: s.files,
        });
      } else if (s.step === "failed") {
        await recordDeploy({ projectId: project.id, kind, status: "failed", message: s.message });
      }
      await loadDeploys();
    } catch (e) {
      console.error("could not record deploy:", e);
    }
  }

  async function deploy() {
    if (!config) return;
    error = null;
    currentKind = "deploy";
    step = { step: "committing" };
    try {
      await deployProject(config);
    } catch (e) {
      const failed: DeployStep = { step: "failed", message: String(e) };
      step = failed;
      currentKind = null;
      void record("deploy", failed);
    }
  }

  async function importFiles() {
    if (!config) return;
    error = null;
    currentKind = null; // imports are not recorded as deploys
    step = { step: "downloading", percent: 0 };
    try {
      await pullProject(config, "import");
    } catch (e) {
      step = { step: "failed", message: String(e) };
    }
  }

  async function commit(event: SubmitEvent) {
    event.preventDefault();
    if (!config) return;
    committing = true;
    error = null;
    try {
      await commitProject(config, commitMessage);
      commitMessage = "";
      await refreshStatus();
    } catch (e) {
      error = String(e);
    } finally {
      committing = false;
    }
  }

  function onRollback(commitId: string) {
    if (!config) return;
    showHistory = false;
    error = null;
    currentKind = "rollback";
    step = { step: "checking_out" };
    rollbackProject(config, commitId).catch((e) => {
      const failed: DeployStep = { step: "failed", message: String(e) };
      step = failed;
      currentKind = null;
      void record("rollback", failed);
    });
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
        No local folder on this device. Add one under <strong>Overview → Local
        folder</strong> to deploy and commit from here. You can still see the
        shared deploy history below.
      </p>
    </div>
  {:else}
    <div class="card actions-card">
      <div class="deploy-row">
        <button class="primary" onclick={deploy} disabled={running}>
          {running ? "Working…" : "Deploy"}
        </button>
        <button class="ghost" onclick={importFiles} disabled={running} title="Pull the server's files into the local folder">
          Import from server
        </button>
        <button class="ghost" onclick={() => (showHistory = true)} disabled={running}>
          History & rollback
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

      <form class="commit" onsubmit={commit}>
        {#if gitStatus?.dirty}
          <p class="changes muted">
            {gitStatus.changed.length}
            {gitStatus.changed.length === 1 ? "uncommitted change" : "uncommitted changes"}
          </p>
          <div class="commit-row">
            <input bind:value={commitMessage} placeholder="Commit message" autocomplete="off" disabled={committing} />
            <button type="submit" class="ghost" disabled={committing}>Commit</button>
          </div>
        {:else}
          <p class="muted small">Working tree clean.</p>
        {/if}
      </form>
    </div>
  {/if}

  <h3 class="timeline-title">Deploy history</h3>
  {#if deploys.length === 0}
    <p class="muted center empty">No deploys recorded yet.</p>
  {:else}
    <ul class="timeline">
      {#each deploys as d (d.id)}
        <li>
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
        </li>
      {/each}
    </ul>
  {/if}
</div>

{#if showHistory && config}
  <HistoryView
    project={config}
    {onRollback}
    onChanged={refreshStatus}
    onClose={() => (showHistory = false)}
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

  .commit {
    margin-top: 14px;
    border-top: 1px solid var(--border);
    padding-top: 12px;
  }

  .changes {
    margin: 0 0 8px;
    font-size: 12px;
  }

  .commit-row {
    display: flex;
    gap: 8px;
  }

  .commit-row input {
    flex: 1;
  }

  .small {
    font-size: 12px;
    margin: 0;
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

  .timeline li {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 12px 14px;
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
