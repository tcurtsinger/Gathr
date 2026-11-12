import { fileURLToPath } from 'node:url';
import tailwindcss from '@tailwindcss/vite';
import react from '@vitejs/plugin-react';
import { playwright } from '@vitest/browser-playwright';
import { defineConfig } from 'vitest/config';
import { serviceProxy } from './proxy.ts';

const uiPort = Number(process.env.GATHR_UI_PORT ?? 47112);
// GATHR_DATA is the service's --data folder; without it there's no service to forward to.
const data = process.env.GATHR_DATA;
const mac = process.platform === 'darwin';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // The registry files import each other as @/… (0030).
  resolve: { alias: { '@': fileURLToPath(new URL('./src/registry', import.meta.url)) } },
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  server: {
    host: '127.0.0.1',
    port: uiPort,
    strictPort: true,
    proxy: data ? serviceProxy(data, uiPort) : {},
    // tokens.css's font is in designs/, outside this folder.
    fs: { allow: ['.', '../designs'] },
  },
  // Bundled before the tests start: discovering them mid-run reloads a test (seen in CI).
  optimizeDeps: {
    include: [
      'react',
      'react-dom/client',
      'react/jsx-dev-runtime',
      'vitest-browser-react',
      '@base-ui/react/preview-card',
    ],
  },
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: 'ui',
          include: ['src/**/*.test.{ts,tsx}'],
          setupFiles: ['src/test.setup.ts'],
          browser: {
            enabled: true,
            headless: true,
            // WebView2 is Edge's engine and WKWebView is WebKit's (0022).
            provider: playwright({ launchOptions: mac ? {} : { channel: 'msedge' } }),
            instances: [{ browser: mac ? 'webkit' : 'chromium' }],
          },
        },
      },
      // The dev server itself runs in Node.
      { extends: true, test: { name: 'dev server', include: ['*.test.ts'], environment: 'node' } },
    ],
  },
});
