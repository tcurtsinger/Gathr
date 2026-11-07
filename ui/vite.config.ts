import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import react from '@vitejs/plugin-react';
import { playwright } from '@vitest/browser-playwright';
import type { ProxyOptions } from 'vite';
import { defineConfig } from 'vitest/config';

// The dev server forwards /api to the member's own service and adds its token there,
// so the page never holds the token (0004). GATHR_DATA is the service's --data folder.
function service(): Record<string, ProxyOptions> {
  const data = process.env.GATHR_DATA;
  if (!data) return {};
  // Read on every request: the token changes when the service restarts.
  const info = () => JSON.parse(readFileSync(join(data, 'service.json'), 'utf8'));
  const sign = (req: { removeHeader(name: string): void; setHeader(name: string, value: string): void }) => {
    // The service refuses foreign origins; the dev server's own origin is this proxy's.
    req.removeHeader('origin');
    req.setHeader('Authorization', `Bearer ${info().token}`);
  };
  return {
    '/api': {
      target: `http://127.0.0.1:${info().port}`,
      ws: true,
      changeOrigin: true,
      configure(proxy) {
        proxy.on('proxyReq', sign);
        proxy.on('proxyReqWs', sign);
      },
    },
  };
}

const mac = process.platform === 'darwin';

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  server: {
    host: '127.0.0.1',
    port: Number(process.env.GATHR_UI_PORT ?? 47112),
    strictPort: true,
    proxy: service(),
  },
  // Bundled before the tests start: discovering them mid-run reloads a test (seen in CI).
  optimizeDeps: { include: ['react', 'react-dom/client', 'react/jsx-dev-runtime', 'vitest-browser-react'] },
  test: {
    browser: {
      enabled: true,
      headless: true,
      // WebView2 is Edge's engine and WKWebView is WebKit's (0022).
      provider: playwright({ launchOptions: mac ? {} : { channel: 'msedge' } }),
      instances: [{ browser: mac ? 'webkit' : 'chromium' }],
    },
  },
});
