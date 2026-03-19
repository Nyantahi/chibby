import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { BrowserRouter } from 'react-router-dom';
import AddProject from '../../components/AddProject';
import * as api from '../../services/api';
import { open } from '@tauri-apps/plugin-dialog';
import type { DetectedScript, Pipeline } from '../../types';

vi.mock('../../services/api');
vi.mock('@tauri-apps/plugin-dialog');

const mockScripts: DetectedScript[] = [
  { file_name: 'package.json', file_path: '/app/package.json', script_type: 'PackageJson' },
  { file_name: 'Makefile', file_path: '/app/Makefile', script_type: 'Makefile' },
];

const mockPipeline: Pipeline = {
  name: 'my-app',
  stages: [
    { name: 'install', commands: ['npm install'], backend: 'local', fail_fast: true },
    { name: 'build', commands: ['npm run build'], backend: 'local', fail_fast: true },
  ],
};

function renderAddProject() {
  return render(
    <BrowserRouter>
      <AddProject />
    </BrowserRouter>
  );
}

describe('AddProject', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    // Default mock for getGithubWorkflows (used in handleScan)
    vi.mocked(api.getGithubWorkflows).mockResolvedValue([]);
  });

  describe('Initial select step', () => {
    it('renders page title', () => {
      renderAddProject();

      expect(screen.getByRole('heading', { name: 'Add Project' })).toBeInTheDocument();
    });

    it('shows repository path input', () => {
      renderAddProject();

      expect(screen.getByLabelText(/repository path/i)).toBeInTheDocument();
    });

    it('shows project name input', () => {
      renderAddProject();

      expect(screen.getByLabelText(/project name/i)).toBeInTheDocument();
    });

    it('shows scan button', () => {
      renderAddProject();

      expect(screen.getByRole('button', { name: /scan repository/i })).toBeInTheDocument();
    });

    it('shows browse button', () => {
      renderAddProject();

      expect(screen.getByRole('button', { name: /browse/i })).toBeInTheDocument();
    });

    it('allows entering repository path', async () => {
      const user = userEvent.setup();
      renderAddProject();

      const input = screen.getByLabelText(/repository path/i);
      await user.type(input, '/Users/dev/my-project');

      expect(input).toHaveValue('/Users/dev/my-project');
    });

    it('allows entering project name', async () => {
      const user = userEvent.setup();
      renderAddProject();

      const input = screen.getByLabelText(/project name/i);
      await user.type(input, 'My Custom Name');

      expect(input).toHaveValue('My Custom Name');
    });

    it('opens folder picker on browse click', async () => {
      const user = userEvent.setup();
      vi.mocked(open).mockResolvedValue('/Users/dev/selected-folder');

      renderAddProject();

      await user.click(screen.getByRole('button', { name: /browse/i }));

      expect(open).toHaveBeenCalledWith({
        directory: true,
        multiple: false,
        title: 'Select repository folder',
      });
    });

    it('sets path from folder picker', async () => {
      const user = userEvent.setup();
      vi.mocked(open).mockResolvedValue('/Users/dev/selected-folder');

      renderAddProject();

      await user.click(screen.getByRole('button', { name: /browse/i }));

      await waitFor(() => {
        expect(screen.getByLabelText(/repository path/i)).toHaveValue('/Users/dev/selected-folder');
      });
    });
  });

  describe('Scan and detect step', () => {
    it('shows error when path is empty', async () => {
      const user = userEvent.setup();
      renderAddProject();

      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      expect(screen.getByText('Please enter a repository path.')).toBeInTheDocument();
    });

    it('calls detectScripts and generatePipeline', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockResolvedValue(mockScripts);
      vi.mocked(api.generatePipeline).mockResolvedValue(mockPipeline);

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/Users/dev/my-app');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      await waitFor(() => {
        expect(api.detectScripts).toHaveBeenCalledWith('/Users/dev/my-app');
      });
      expect(api.generatePipeline).toHaveBeenCalled();
    });

    it('shows detected scripts', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockResolvedValue(mockScripts);
      vi.mocked(api.generatePipeline).mockResolvedValue(mockPipeline);

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/Users/dev/my-app');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      await waitFor(() => {
        expect(screen.getByText('package.json')).toBeInTheDocument();
      });
      // Makefile appears twice (name and type), use getAllByText
      const makefileElements = screen.getAllByText('Makefile');
      expect(makefileElements.length).toBeGreaterThan(0);
    });

    it('shows generated pipeline stages', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockResolvedValue(mockScripts);
      vi.mocked(api.generatePipeline).mockResolvedValue(mockPipeline);

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/Users/dev/my-app');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      // After scan, component moves to 'source' step - click Auto-detect to see stages
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /auto-detect/i })).toBeInTheDocument();
      });
      await user.click(screen.getByRole('button', { name: /auto-detect/i }));

      await waitFor(() => {
        expect(screen.getByText('install')).toBeInTheDocument();
      });
      expect(screen.getByText('build')).toBeInTheDocument();
    });

    it('shows no scripts message when none found', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockResolvedValue([]);
      vi.mocked(api.generatePipeline).mockResolvedValue({ name: 'empty', stages: [] });

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/Users/dev/my-app');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      // Component shows "Found 0 build files" when no scripts detected
      await waitFor(() => {
        expect(screen.getByText(/found 0 build files?/i)).toBeInTheDocument();
      });
    });
  });

  describe('Save step', () => {
    it('saves pipeline and adds project', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockResolvedValue(mockScripts);
      vi.mocked(api.generatePipeline).mockResolvedValue(mockPipeline);
      vi.mocked(api.savePipeline).mockResolvedValue(undefined);
      vi.mocked(api.addProject).mockResolvedValue({
        id: '123',
        name: 'my-app',
        path: '/Users/dev/my-app',
        added_at: '2026-03-17',
      });

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/Users/dev/my-app');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      // Step 2: Source - click Auto-detect
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /auto-detect/i })).toBeInTheDocument();
      });
      await user.click(screen.getByRole('button', { name: /auto-detect/i }));

      // Step 3: Configure - verify stages and continue
      await waitFor(() => {
        expect(screen.getByText('install')).toBeInTheDocument();
      });
      await user.click(screen.getByRole('button', { name: /continue/i }));

      // Step 4: Review - create project
      await waitFor(() => {
        expect(screen.getByRole('button', { name: /create project/i })).toBeInTheDocument();
      });
      await user.click(screen.getByRole('button', { name: /create project/i }));

      await waitFor(() => {
        expect(api.savePipeline).toHaveBeenCalled();
      });
      expect(api.addProject).toHaveBeenCalled();
    });

    it('allows skipping pipeline', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockResolvedValue(mockScripts);
      vi.mocked(api.generatePipeline).mockResolvedValue(mockPipeline);
      vi.mocked(api.addProject).mockResolvedValue({
        id: '123',
        name: 'my-app',
        path: '/Users/dev/my-app',
        added_at: '2026-03-17',
      });

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/Users/dev/my-app');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      await waitFor(() => {
        expect(screen.getByRole('button', { name: /skip pipeline/i })).toBeInTheDocument();
      });

      await user.click(screen.getByRole('button', { name: /skip pipeline/i }));

      await waitFor(() => {
        expect(api.addProject).toHaveBeenCalled();
      });
      expect(api.savePipeline).not.toHaveBeenCalled();
    });
  });

  describe('Error handling', () => {
    it('shows error when scan fails', async () => {
      const user = userEvent.setup();
      vi.mocked(api.detectScripts).mockRejectedValue(new Error('Directory not found'));

      renderAddProject();

      await user.type(screen.getByLabelText(/repository path/i), '/invalid/path');
      await user.click(screen.getByRole('button', { name: /scan repository/i }));

      await waitFor(() => {
        expect(screen.getByText(/directory not found/i)).toBeInTheDocument();
      });
    });
  });
});
