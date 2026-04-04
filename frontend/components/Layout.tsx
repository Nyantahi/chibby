import { useState, useEffect } from 'react';
import { Outlet, NavLink } from 'react-router-dom';
import { FolderGit2, PlusCircle, Settings, Layers } from 'lucide-react';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { getAppVersion } from '../services/api';

function ChibbyLogo({ size = 22 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
      <rect x="0" y="0" width="200" height="200" rx="40" fill="currentColor" />
      <path
        d="M124 42 L76 42 Q56 42 56 62 L56 148 Q56 168 76 168 L124 168"
        fill="none"
        stroke="var(--color-bg, #0d1117)"
        strokeWidth="14"
        strokeLinecap="round"
      />
      <polygon
        points="122,52 88,104 110,104 78,156 148,92 122,92"
        fill="var(--color-bg, #0d1117)"
      />
    </svg>
  );
}

function Layout() {
  useKeyboardShortcuts();
  const [version, setVersion] = useState<string>('');

  useEffect(() => {
    getAppVersion().then(setVersion).catch(() => setVersion('unknown'));
  }, []);

  return (
    <div className="app-layout">
      <aside className="sidebar">
        <div className="sidebar-header">
          <ChibbyLogo size={22} />
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
          <NavLink to="/templates" className="nav-link">
            <Layers size={16} />
            <span>Templates</span>
          </NavLink>
        </nav>

        <div className="sidebar-footer">
          <NavLink to="/settings" className="nav-link">
            <Settings size={16} />
            <span>Settings</span>
          </NavLink>
          <span className="version-label">v{version}</span>
        </div>
      </aside>

      <main className="main-content">
        <Outlet />
      </main>
    </div>
  );
}

export default Layout;
