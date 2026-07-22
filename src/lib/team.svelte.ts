// The currently active team, persisted across restarts.

import type { Team } from "./cloud";

const STORAGE_KEY = "feather.activeTeam";

export const teamState = $state({
  activeTeamId: localStorage.getItem(STORAGE_KEY),
  activeTeamName: null as string | null,
});

export function setActiveTeam(team: Team): void {
  teamState.activeTeamId = team.id;
  teamState.activeTeamName = team.name;
  localStorage.setItem(STORAGE_KEY, team.id);
}

export function clearActiveTeam(): void {
  teamState.activeTeamId = null;
  teamState.activeTeamName = null;
  localStorage.removeItem(STORAGE_KEY);
}
