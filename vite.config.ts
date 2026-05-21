import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    hmr: {
      overlay: false,
    },
  },
  envPrefix: ['VITE_', 'TAURI_'],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './frontend'),
    },
  },
  build: {
    // Tauri ships modern WebViews on every platform (WebKit on macOS/Linux
    // via webkit2gtk, Chromium-based WebView2 on Windows). Targeting `esnext`
    // lets esbuild emit the actual syntax used by react-router 7 et al.
    // without the destructuring-transform errors that older floors trigger.
    target: 'esnext',
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    sourcemap: !!process.env.TAURI_DEBUG,
    outDir: 'dist',
  },
});
