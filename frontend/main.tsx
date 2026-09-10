import React from 'react';
import ReactDOM from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import App from './App';
import { getPref, PREF_THEME, type Theme } from './services/prefs';
import './styles/index.css';

// Apply the saved theme before first paint to avoid a flash of the wrong theme.
const theme = getPref<Theme>(PREF_THEME, 'dark');
document.documentElement.setAttribute('data-theme', theme);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <BrowserRouter>
      <App />
    </BrowserRouter>
  </React.StrictMode>
);
