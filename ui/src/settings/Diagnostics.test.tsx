// The service is the edge here: fetch answers a trace as the service does for a refused request.
import { afterEach, expect, test, vi } from 'vitest';
import { render } from 'vitest-browser-react';
import { Diagnostics } from './Diagnostics';

afterEach(() => vi.unstubAllGlobals());

test("a refusal's reference shows the reason its notice gave", async () => {
  const refused = {
    attrs: {
      error: 'Unarchive failed',
      'http.request.method': 'POST',
      'http.response.status_code': 409,
      reason: "it isn't archived",
      'url.path': '/api/chats/made-up-chat/unarchive',
    },
    conversation_id: null,
    duration_us: 408,
    error_type: '409',
    name: 'gathr.api.request',
    offset_ms: 0,
    parent_span_id: null,
    phase: 'point',
    span_id: null,
    status: 'error',
    time: '2026-11-18T11:14:33.304Z',
    trace_id: '95fb3d5e57b70aa976dacbfa95abbaae',
    ts: 1795000473303557,
  };
  vi.stubGlobal('fetch', async (path: string) =>
    path === '/api/trace/95fb3d5e'
      ? new Response(JSON.stringify({ reference: '95fb3d5e', events: [refused] }))
      : new Response('{}'),
  );
  const screen = await render(<Diagnostics reference="95fb3d5e" />);
  await expect.element(screen.getByText('Request failed (409)')).toBeVisible();
  await expect.element(screen.getByText("it isn't archived", { exact: true })).toBeVisible();
});
