import { Outlet, NavLink } from 'react-router-dom';
import { FolderGit2, PlusCircle, Rocket, Settings } from 'lucide-react';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';

function Layout() {
  useKeyboardShortcuts();

  return (
    <div className="app-layout">
      <aside className="sidebar">
        <div className="sidebar-header">
          <Rocket size={22} />
          <h1 className="sidebar-title">Chibby</h1>
        </div>

        <nav className="sidebar-nav">
          <NavLink to="/" end className="nav-link">
            <FolderGit2 size={16} />
            <span>Projects</span>
          </NavLink>
          <NavLink to="/add-project" className="nav-link">
            <PlusCircle size={16} />
            <span>Add Project</span>
          </NavLink>
        </nav>

        <div className="sidebar-footer">
          <NavLink to="/settings" className="nav-link">
            <Settings size={16} />
            <span>Settings</span>
          </NavLink>
          <span className="version-label">v0.1.0</span>
        </div>
      </aside>

      <main className="main-content">
        <Outlet />
      </main>
    </div>
  );
}

export default Layout;
