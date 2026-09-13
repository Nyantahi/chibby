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
    rollupOptions: {
      output: {
        // Keep the framework in its own chunk: it rarely changes, so it stays
        // cached across app updates while our own code (and the lazy route
        // chunks) are what actually turn over. Rolldown (Vite 8) wants the
        // function form of manualChunks, not the object form.
        manualChunks(id) {
          if (/node_modules\/(react|react-dom|react-router|react-router-dom|scheduler)\//.test(id)) {
            return 'react-vendor';
          }
        },
      },
    },
  },
});
