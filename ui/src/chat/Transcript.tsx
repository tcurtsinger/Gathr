import type { Message, ToolBody } from '../api';
import { Notice } from '../components/Notice';
import type { Row } from './rows';
import './transcript.css';

export interface NoticeActions {
  diagnostics: (turn?: string) => void;
  signIn: () => void;
  retry: (turn: string) => void;
  carryOn: () => void;
}

/** The chat's rows, oldest first. Off-screen rows skip layout and paint (content-visibility). */
export function Transcript({ rows, actions }: { rows: Row[]; actions: NoticeActions }) {
  return rows.map((row) => <RowView key={key(row)} row={row} actions={actions} />);
}

function key(row: Row) {
  switch (row.kind) {
    case 'message':
      return row.message.id;
    case 'tools':
      return row.id;
    case 'streaming':
      return `s-${row.turn}`;
    case 'notice':
      return `n-${row.turn.id}`;
  }
}

function RowView({ row, actions }: { row: Row; actions: NoticeActions }) {
  switch (row.kind) {
    case 'message':
      return <Text message={row.message} />;
    case 'streaming':
      return <Reply text={row.text} />;
    case 'tools':
      return <Tools tools={row.tools} />;
    case 'notice':
      return <TurnNotice row={row} actions={actions} />;
  }
}

function Text({ message }: { message: Message }) {
  const text = message.body as string;
  if (message.role === 'user') return <div className="message user">{text}</div>;
  return <Reply text={text} />;
}

function Reply({ text }: { text: string }) {
  return <div className="message reply">{text}</div>;
}

const ran = (n: number) => (n === 1 ? '1 tool call' : `${n} tool calls`);

function Tools({ tools }: { tools: ToolBody[] }) {
  const running = tools.filter((t) => t.status === 'running').length;
  return (
    <details className="message tools">
      <summary>
        {ran(tools.length)}
        {running > 0 && ', running'}
      </summary>
      <ul>
        {tools.map((t, i) => (
          // Tool rows never reorder, so their position is a stable key.
          // biome-ignore lint/suspicious/noArrayIndexKey: see above
          <li key={i}>
            <span>{t.tool}</span>
            <span className="muted">{t.status === 'ok' ? '' : t.status}</span>
            <span className="muted">{t.duration_ms === undefined ? '' : seconds(t.duration_ms)}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}

function seconds(ms: number) {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`;
}

function TurnNotice({ row, actions }: { row: Extract<Row, { kind: 'notice' }>; actions: NoticeActions }) {
  const { turn } = row;
  if (turn.status === 'stopped') {
    return (
      <Notice
        title="Stopped"
        action={
          <button type="button" onClick={actions.carryOn}>
            Continue
          </button>
        }
      />
    );
  }
  if (turn.status === 'waiting_sign_in') {
    return (
      <Notice
        title="Waiting for sign-in"
        action={
          <button type="button" onClick={actions.signIn}>
            Sign in
          </button>
        }
      />
    );
  }
  return (
    <Notice
      danger
      title={row.replied ? 'Reply failed' : 'Send failed'}
      reference={{ ref: turn.ref, open: () => actions.diagnostics(turn.id) }}
      links={<DiagnosticsLink open={() => actions.diagnostics(turn.id)} />}
      action={
        <button type="button" onClick={() => actions.retry(turn.id)}>
          Retry
        </button>
      }
    />
  );
}

export function DiagnosticsLink({ open }: { open: () => void }) {
  return (
    <button type="button" className="link" onClick={open}>
      Diagnostics
    </button>
  );
}
