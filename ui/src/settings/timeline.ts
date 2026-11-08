import type { TurnEvent, TurnStatus, TurnTimeline } from '../api';

const statuses: Record<TurnStatus, string> = {
  done: 'Completed',
  failed: 'Failed',
  stopped: 'Stopped',
  running: 'Running',
  queued: 'Queued',
  waiting_sign_in: 'Waiting for sign-in',
};

export const said = (status: TurnStatus) => statuses[status] ?? status;

export function seconds(ms: number) {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} s`;
  return `${Math.floor(ms / 60_000)}m ${Math.round((ms % 60_000) / 1000)}s`;
}

const usage: [string, string][] = [
  ['Input tokens', 'gen_ai.usage.input_tokens'],
  ['Output tokens', 'gen_ai.usage.output_tokens'],
  ['Cache read', 'gen_ai.usage.cache_read.input_tokens'],
  ['Cache write', 'gen_ai.usage.cache_creation.input_tokens'],
];

/** A turn's totals as the engine reported them, from the event that ended it. */
export function facts(turn: TurnTimeline) {
  const end = turn.events.findLast((e) => e.attrs?.['gen_ai.usage.output_tokens'] !== undefined);
  const tokens = usage
    .filter(([, key]) => typeof end?.attrs?.[key] === 'number')
    .map(([label, key]) => [label, end?.attrs?.[key] as number] as const);
  const tools = turn.events.filter((e) => e.name.startsWith('execute_tool ')).length;
  const duration = end?.duration_ms ?? null;
  const summary = [tools === 1 ? '1 tool' : `${tools} tools`, duration === null ? '' : seconds(duration)]
    .filter(Boolean)
    .join(', ');
  return { tokens, given: given(turn), duration, summary };
}

const parts: [string, string][] = [
  ['Briefing', 'gathr.context.briefing'],
  ['Message', 'gathr.context.ask'],
];

/** What Gathr gave the turn: characters, and Gathr's own token estimate (0020). */
function given(turn: TurnTimeline) {
  const start = turn.events.find((e) => e.name.startsWith('invoke_agent ') && e.phase === 'start');
  return parts.flatMap(([label, key]) => {
    const chars = start?.attrs?.[`${key}.chars`];
    const estimate = start?.attrs?.[`${key}.tokens_estimate`];
    if (typeof chars !== 'number' || typeof estimate !== 'number') return [];
    return [[label, `${chars.toLocaleString()} chars, ~${estimate.toLocaleString()} tokens`] as const];
  });
}

export interface Step {
  kind: 'event';
  key: string;
  label: string;
  at: number;
  duration: number | null;
  failed: boolean;
}

export type Line = Step | { kind: 'tools'; key: string; events: Step[] };

/** The timeline's lines; runs of more than three tool calls fold into one line. */
export function steps(events: TurnEvent[]): Line[] {
  const out: Line[] = [];
  let run: Step[] = [];
  const flush = () => {
    if (run.length > 3) out.push({ kind: 'tools', key: `tools-${run[0].key}`, events: run });
    else out.push(...run);
    run = [];
  };
  events.forEach((e, i) => {
    const step: Step = {
      kind: 'event',
      key: `${i}`,
      label: label(e),
      at: e.at_ms,
      duration: e.duration_ms,
      failed: e.status === 'error',
    };
    if (e.name.startsWith('execute_tool ')) {
      run.push(step);
      return;
    }
    flush();
    out.push(step);
  });
  flush();
  return out;
}

// Each event's line when it went well, and its subject when it failed ("Engine start failed").
const known: Record<string, [string, string]> = {
  'gathr.chat.send': ['Sent', 'Send'],
  'gathr.engine.start': ['Engine started', 'Engine start'],
  'gathr.turn.first_token': ['First text', 'First text'],
  'gathr.turn.steer': ['Message sent into the turn', 'Message into the turn'],
  'gathr.api.request': ['Request', 'Request'],
  'gathr.engine.sign_in': ['Signed in', 'Sign in'],
};

function label(e: TurnEvent): string {
  const failed = e.status === 'error' ? ` failed${e.error ? ` (${e.error})` : ''}` : '';
  if (e.name.startsWith('execute_tool ')) return `${e.name.slice(13)}${failed}`;
  const tool = e.name === 'gathr.tool.call' ? e.attrs?.['gen_ai.tool.name'] : undefined;
  if (typeof tool === 'string') return `Gathr's ${tool}${e.status === 'error' ? ' failed' : ''}`;
  if (e.name.startsWith('invoke_agent ')) {
    if (e.phase === 'start') return 'Turn started';
    const status = e.attrs?.['gathr.turn.status'];
    return `Turn ended${typeof status === 'string' ? `: ${said(status as TurnStatus)}` : ''}`;
  }
  const [ok, subject] = known[e.name] ?? [e.name, e.name];
  if (typeof e.attrs?.ended === 'string') return `${subject} ${e.attrs.ended}`;
  return failed ? `${subject}${failed}` : ok;
}
