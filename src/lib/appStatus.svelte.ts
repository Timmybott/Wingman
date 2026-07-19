// Small cross-component app status (footer etc.), shared via Svelte 5 runes.

export interface LastDeploy {
  projectName: string;
  at: Date;
  files: number;
}

export interface GitStatus {
  projectName: string;
  commitsSince: number;
}

export const appStatus = $state({
  lastDeploy: null as LastDeploy | null,
  /** "N commits since last deploy" for the most recently touched project. */
  gitStatus: null as GitStatus | null,
});
