// The window's own errors and timings go to the service's event record (0011, 0019), so
// `gathr diag` finds them beside the service's. Reporting never throws: if the service is
// down, the console is the only record left.

export interface UiEvent {
  name: string;
  duration_ms?: number;
  error?: string;
  attrs?: Record<string, unknown>;
}

export function report(event: UiEvent) {
  fetch('/api/ui/events', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(event),
  }).catch(() => console.error('Report failed', event));
}

/** Reports how long `name` took, from `start` to the next frame the browser draws. */
export function timeToPaint(name: string, start: number, attrs?: Record<string, unknown>) {
  requestAnimationFrame(() =>
    setTimeout(() => report({ name, duration_ms: performance.now() - start, attrs })),
  );
}

addEventListener('error', (e) =>
  report({ name: 'gathr.ui.error', error: 'uncaught', attrs: { message: e.message } }),
);
addEventListener('unhandledrejection', (e) =>
  report({ name: 'gathr.ui.error', error: 'unhandled_rejection', attrs: { message: String(e.reason) } }),
);
