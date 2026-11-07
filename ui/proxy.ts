import { readFileSync } from 'node:fs';
import type { ClientRequest, IncomingMessage, ServerResponse } from 'node:http';
import { join } from 'node:path';
import type { Duplex } from 'node:stream';
import type { ProxyOptions } from 'vite';

/** The dev server forwards /api to the member's own service and adds its token there, so the
 * page never holds the token (0004). `data` is the service's --data folder.
 *
 * Adding the token makes the proxy a way in, so it does the service's Origin check first: a
 * page from another site gets nothing, while a request with no Origin (not from a page) passes. */
export function serviceProxy(data: string, uiPort: number): Record<string, ProxyOptions> {
  const own = [`http://127.0.0.1:${uiPort}`, `http://localhost:${uiPort}`];
  const foreign = (req: IncomingMessage) =>
    req.headers.origin !== undefined && !own.includes(req.headers.origin);
  // Read on every request: the token changes when the service restarts.
  const info = () => JSON.parse(readFileSync(join(data, 'service.json'), 'utf8'));
  const sign = (proxyReq: ClientRequest) => {
    proxyReq.removeHeader('origin');
    proxyReq.setHeader('Authorization', `Bearer ${info().token}`);
  };
  return {
    '/api': {
      target: `http://127.0.0.1:${info().port}`,
      ws: true,
      changeOrigin: true,
      configure(proxy) {
        proxy.on('proxyReq', (proxyReq: ClientRequest, req: IncomingMessage, res: ServerResponse) => {
          if (!foreign(req)) return sign(proxyReq);
          proxyReq.destroy();
          res.writeHead(403).end();
        });
        proxy.on('proxyReqWs', (proxyReq: ClientRequest, req: IncomingMessage, socket: Duplex) => {
          if (!foreign(req)) return sign(proxyReq);
          proxyReq.destroy();
          socket.end('HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n');
        });
      },
    },
  };
}
