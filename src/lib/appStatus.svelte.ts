// Small cross-component app status (footer etc.), shared via Svelte 5 runes.

export interface LastDeploy {
  projectName: string;
  at: Date;
  files: number;
}

export const appStatus = $state({
  lastDeploy: null as LastDeploy | null,
});
