import { useState } from 'react';
import { CopyButton } from '../settings/SignIn';

/** What an AI app on this PC runs to connect: Gathr's command line and its arguments. */
export type Connect = { program: string; args: string[] };

type App = 'claude' | 'codex' | 'other';

const apps: { app: App; name: string }[] = [
  { app: 'claude', name: 'Claude Code' },
  { app: 'codex', name: 'Codex' },
  { app: 'other', name: 'Another app' },
];

/** A word as a command line takes it: in quotes when it holds anything but a plain path's characters. */
const word = (text: string) => (/^[\w./:\\-]+$/.test(text) ? text : `"${text}"`);

/**
 * Connect an app: the one line to paste into an AI app on this PC, which adds Gathr's command line as an MCP server
 * for all its projects (Claude Code's user scope; Codex's own config). Another app gets the program and its arguments.
 */
export function ConnectApp({ connect }: { connect: Connect | null }) {
  const [app, setApp] = useState<App>('claude');
  const run = connect && [`"${connect.program}"`, ...connect.args.map(word)].join(' ');
  const line =
    app === 'claude'
      ? `claude mcp add --transport stdio --scope user gathr -- ${run}`
      : `codex mcp add gathr -- ${run}`;
  const field = (label: string, text: string) => (
    <div className="agent-field">
      <span className="tool-label">{label}</span>
      <div className="tool-redirect">
        <code>{text}</code>
        <CopyButton text={text} label="Copy" />
      </div>
    </div>
  );
  return (
    <article className="skill-page connect-app">
      <h1>Connect an app</h1>
      <p className="muted">
        {app === 'other'
          ? 'Give the app this program and its arguments as an MCP server.'
          : 'Run this line in a terminal on this PC.'}{' '}
        The app can then send work to your team and add skills, tools, agents and projects.
      </p>
      <fieldset className="plugins-kind connect-apps" aria-label="App">
        {apps.map((a) => (
          <button key={a.app} type="button" aria-pressed={app === a.app} onClick={() => setApp(a.app)}>
            {a.name}
          </button>
        ))}
      </fieldset>
      {!connect ? (
        <p className="plugin-problem">Gathr’s command line isn’t on this PC.</p>
      ) : app === 'other' ? (
        <div className="connect-fields">
          {field('Program', connect.program)}
          {field('Arguments', connect.args.map(word).join(' '))}
        </div>
      ) : (
        <div className="tool-redirect">
          <code>{line}</code>
          <CopyButton text={line} label="Copy" />
        </div>
      )}
      <p className="muted">Each app says its own name. What it adds in Gathr says who added it.</p>
    </article>
  );
}
