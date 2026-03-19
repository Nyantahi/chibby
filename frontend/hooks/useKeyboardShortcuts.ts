import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

/**
 * Global keyboard shortcuts for navigation.
 *
 * Cmd/Ctrl + 1 → Dashboard
 * Cmd/Ctrl + 2 → Projects
 * Cmd/Ctrl + N → Add Project
 * Cmd/Ctrl + , → Settings
 */
export function useKeyboardShortcuts() {
  const navigate = useNavigate();

  useEffect(() => {
    function handler(e: KeyboardEvent) {
      const mod = e.metaKey || e.ctrlKey;
      if (!mod) return;

      // Skip if user is typing in an input/textarea
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

      switch (e.key) {
        case '1':
          e.preventDefault();
          navigate('/');
          break;
        case '2':
          e.preventDefault();
          navigate('/projects');
          break;
        case 'n':
        case 'N':
          e.preventDefault();
          navigate('/add-project');
          break;
        case ',':
          e.preventDefault();
          navigate('/settings');
          break;
      }
    }

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [navigate]);
}
