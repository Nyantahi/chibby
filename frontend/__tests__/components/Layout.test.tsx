import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MemoryRouter, Routes, Route } from 'react-router-dom';
import Layout from '../../components/Layout';

function renderLayout(initialPath = '/') {
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Routes>
        <Route element={<Layout />}>
          <Route index element={<div>Dashboard Content</div>} />
          <Route path="/add-project" element={<div>Add Project Content</div>} />
        </Route>
      </Routes>
    </MemoryRouter>
  );
}

describe('Layout', () => {
  it('renders app title', () => {
    renderLayout();

    expect(screen.getByRole('heading', { name: 'Chibby' })).toBeInTheDocument();
  });

  it('renders version label', () => {
    renderLayout();

    expect(screen.getByText('v0.1.0')).toBeInTheDocument();
  });

  it('renders Projects nav link as home', () => {
    renderLayout();

    const projectsLink = screen.getByRole('link', { name: /projects/i });
    expect(projectsLink).toBeInTheDocument();
    expect(projectsLink).toHaveAttribute('href', '/');
  });

  it('renders Add Project nav link', () => {
    renderLayout();

    const addProjectLink = screen.getByRole('link', { name: /add project/i });
    expect(addProjectLink).toBeInTheDocument();
    expect(addProjectLink).toHaveAttribute('href', '/add-project');
  });

  it('renders child content via Outlet', () => {
    renderLayout('/');

    expect(screen.getByText('Dashboard Content')).toBeInTheDocument();
  });

  it('renders different content for different routes', () => {
    renderLayout('/add-project');

    expect(screen.getByText('Add Project Content')).toBeInTheDocument();
  });

  it('has sidebar structure', () => {
    const { container } = renderLayout();

    expect(container.querySelector('.sidebar')).toBeInTheDocument();
    expect(container.querySelector('.main-content')).toBeInTheDocument();
  });

  it('has navigation section', () => {
    const { container } = renderLayout();

    expect(container.querySelector('.sidebar-nav')).toBeInTheDocument();
  });
});
