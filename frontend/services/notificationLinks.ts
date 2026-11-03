import type { NotificationEntry, ProjectInfo } from '../types';

export interface NotificationLink {
  to: string;
  state?: { projectId?: string; tab: string };
}

/** Where an inbox entry leads. A removed project still reaches its run's history. */
export function notificationLink(
  entry: NotificationEntry,
  projects: ProjectInfo[]
): NotificationLink | null {
  const projectId = projects.find((p) => p.project.path === entry.repo_path)?.project.id;

  if (entry.run_id) {
    return { to: `/run/${entry.run_id}`, state: { projectId, tab: 'history' } };
  }
  if (!projectId) return null;
  if (entry.kind === 'release_action') {
    return { to: `/project/${projectId}`, state: { tab: 'release' } };
  }
  return { to: `/project/${projectId}` };
}
