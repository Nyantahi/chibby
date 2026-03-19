import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { BrowserRouter } from 'react-router-dom';
import Projects from '../../components/Projects';
import * as api from '../../services/api';
import type { ProjectInfo } from '../../types';

vi.mock('../../services/api');

const mockProjects: ProjectInfo[] = [
  {
    project: {
      id: '1',
      name: 'my-app',
      path: '/Users/dev/my-app',
      added_at: '2026-03-01T10:00:00Z',
      last_run_status: 'success',
      last_run_at: '2026-03-15T14:30:00Z',
    },
    has_pipeline: true,
  },
  {
    project: {
      id: '2',
      name: 'another-project',
      path: '/Users/dev/another-project',
      added_at: '2026-03-10T12:00:00Z',
    },
    has_pipeline: false,
  },
];

function renderProjects() {
  return render(
    <BrowserRouter>
      <Projects />
    </BrowserRouter>
  );
}

describe('Projects', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(api.getAllRuns).mockResolvedValue([]);
  });

  it('shows loading state initially', () => {
    vi.mocked(api.listProjects).mockImplementation(
      () => new Promise(() => {}) // Never resolves
    );

    renderProjects();

    expect(screen.getByText('Loading projects...')).toBeInTheDocument();
  });

  it('shows empty state when no projects', async () => {
    vi.mocked(api.listProjects).mockResolvedValue([]);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('No projects yet')).toBeInTheDocument();
    });
    expect(screen.getByText('Add Your First Project')).toBeInTheDocument();
  });

  it('renders project list', async () => {
    vi.mocked(api.listProjects).mockResolvedValue(mockProjects);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('my-app')).toBeInTheDocument();
    });
    expect(screen.getByText('another-project')).toBeInTheDocument();
  });

  it('shows project paths', async () => {
    vi.mocked(api.listProjects).mockResolvedValue(mockProjects);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('/Users/dev/my-app')).toBeInTheDocument();
    });
  });

  it('shows pipeline status badge', async () => {
    vi.mocked(api.listProjects).mockResolvedValue(mockProjects);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('Pipeline configured')).toBeInTheDocument();
    });
    expect(screen.getByText('No pipeline')).toBeInTheDocument();
  });

  it('shows last run status', async () => {
    vi.mocked(api.listProjects).mockResolvedValue(mockProjects);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('Success')).toBeInTheDocument();
    });
  });

  it('shows error state', async () => {
    vi.mocked(api.listProjects).mockRejectedValue(new Error('Failed to load'));

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText(/Failed to load/)).toBeInTheDocument();
    });
  });

  it('has link to add project', async () => {
    vi.mocked(api.listProjects).mockResolvedValue([]);

    renderProjects();

    await waitFor(() => {
      const link = screen.getByRole('link', { name: /add your first project/i });
      expect(link).toHaveAttribute('href', '/add-project');
    });
  });

  it('renders project cards as links', async () => {
    vi.mocked(api.listProjects).mockResolvedValue(mockProjects);

    renderProjects();

    await waitFor(() => {
      const projectLink = screen.getByRole('link', { name: /my-app/i });
      expect(projectLink).toHaveAttribute('href', '/project/1');
    });
  });

  it('shows stats bar when projects exist', async () => {
    vi.mocked(api.listProjects).mockResolvedValue(mockProjects);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('Runs Today')).toBeInTheDocument();
    });
    expect(screen.getByText('Success Rate')).toBeInTheDocument();
    expect(screen.getByText('Needs Attention')).toBeInTheDocument();
  });

  it('hides stats bar when no projects', async () => {
    vi.mocked(api.listProjects).mockResolvedValue([]);

    renderProjects();

    await waitFor(() => {
      expect(screen.getByText('No projects yet')).toBeInTheDocument();
    });
    expect(screen.queryByText('Runs Today')).not.toBeInTheDocument();
  });
});
