// The dev proxy against a stand-in service on a free port: only the UI's own pages get the token.
import { createHash } from 'node:crypto';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { createServer as createHttp, request, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createServer, type ViteDevServer } from 'vite';
import { afterAll, beforeAll, expect, test } from 'vitest';
import { serviceProxy } from './proxy';

const token = 'made-up-token';
const seen: string[] = [];
let service: Server;
let ui: ViteDevServer;
let uiPort: number;

beforeAll(async () => {
  service = createHttp((req, res) => {
    seen.push(`GET ${req.headers.authorization ?? 'no token'}`);
    res.end('{}');
  });
  service.on('upgrade', (req, socket) => {
    seen.push(`upgrade ${req.headers.authorization ?? 'no token'}`);
    const key = req.headers['sec-websocket-key'] as string;
    const accept = createHash('sha1').update(`${key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`).digest('base64');
    socket.write(
      `HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: ${accept}\r\n\r\n`,
    );
  });
  await new Promise<void>((done) => service.listen(0, '127.0.0.1', done));
  const data = mkdtempSync(join(tmpdir(), 'gathr-proxy-'));
  const port = (service.address() as AddressInfo).port;
  writeFileSync(join(data, 'service.json'), JSON.stringify({ port, token }));
  uiPort = await freePort();
  ui = await createServer({
    configFile: false,
    logLevel: 'silent',
    server: { host: '127.0.0.1', port: uiPort, strictPort: true, proxy: serviceProxy(data, uiPort) },
  });
  await ui.listen();
});

afterAll(async () => {
  await ui.close();
  service.close();
});

function freePort() {
  const probe = createHttp();
  return new Promise<number>((done) =>
    probe.listen(0, '127.0.0.1', () => {
      const { port } = probe.address() as AddressInfo;
      probe.close(() => done(port));
    }),
  );
}

/** A WebSocket handshake to /api/live from a page at `origin`: the status the proxy answers. */
function handshake(origin: string) {
  return new Promise<number>((done) => {
    const req = request({
      host: '127.0.0.1',
      port: uiPort,
      path: '/api/live',
      headers: {
        Connection: 'Upgrade',
        Upgrade: 'websocket',
        'Sec-WebSocket-Version': '13',
        'Sec-WebSocket-Key': 'dGhlIHNhbXBsZSBub25jZQ==',
        Origin: origin,
      },
    });
    req.on('upgrade', (res, socket) => {
      socket.destroy();
      done(res.statusCode ?? 0);
    });
    req.on('response', (res) => done(res.statusCode ?? 0));
    req.on('error', () => done(0));
    req.end();
  });
}

test('a page from another site gets no live socket and no answer, and never reaches the service', async () => {
  seen.length = 0;
  expect(await handshake('https://attacker.example')).not.toBe(101);
  const answer = await fetch(`http://127.0.0.1:${uiPort}/api/chats`, {
    headers: { Origin: 'https://attacker.example' },
  });
  expect(answer.status).toBe(403);
  expect(seen).toEqual([]);
});

test("the UI's own page gets the live socket, signed with the service's token", async () => {
  seen.length = 0;
  expect(await handshake(`http://127.0.0.1:${uiPort}`)).toBe(101);
  expect(seen).toEqual([`upgrade Bearer ${token}`]);
});
