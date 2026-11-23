import { useState } from 'react';
import { Dropdown } from '../components/Dropdown';
import { Page, Row } from './Row';
import { CopyButton } from './SignIn';
import { useSettings } from './saved';

/** What an AI app on this PC runs to connect: Gathr's command line and its arguments. */
export type Connect = { program: string; args: string[] };

type App = 'claude' | 'codex' | 'other';

const apps: { value: App; label: string }[] = [
  { value: 'claude', label: 'Claude Code' },
  { value: 'codex', label: 'Codex' },
  { value: 'other', label: 'Another app' },
];

/** A word as a command line takes it: in quotes when it holds anything but a plain path's characters. */
const word = (text: string) => (/^[\w./:\\-]+$/.test(text) ? text : `"${text}"`);

/**
 * Settings > Connect: the one line to run so an AI app on this PC can use Gathr. It adds Gathr's command line as an
 * MCP server for all the app's projects (Claude Code's user scope; Codex's own config). Another app gets the program
 * and its arguments.
 */
export function Connect() {
  const { saved } = useSettings();
  const [app, setApp] = useState<App>('claude');
  const connect = saved?.connect ?? null;
  const run = connect && [`"${connect.program}"`, ...connect.args.map(word)].join(' ');
  const line =
    app === 'claude'
      ? `claude mcp add --transport stdio --scope user gathr -- ${run}`
      : `codex mcp add gathr -- ${run}`;
  const give = (text: string, label?: string) => (
    <div className="connect-give">
      {label && <span className="setting-name">{label}</span>}
      <div>
        <code>{text}</code>
        <CopyButton text={text} label="Copy" />
      </div>
    </div>
  );
  return (
    <Page title="Connect" line="Let an AI app on this PC add skills, tools, agents and projects.">
      {saved && (
        <>
          <div className="settings-rows">
            <Row
              name={<label htmlFor="connect-app">App</label>}
              line={
                app === 'other'
                  ? 'Give the app this program and its arguments as an MCP server.'
                  : 'Run this line in a terminal on this PC.'
              }
              lineId="connect-line"
            >
              <Dropdown
                id="connect-app"
                aria-describedby="connect-line"
                value={app}
                onChange={(v) => setApp(v as App)}
                choices={apps}
              />
            </Row>
          </div>
          {!connect ? (
            <p className="connect-note">Gathr’s command line isn’t on this PC.</p>
          ) : app === 'other' ? (
            <>
              {give(connect.program, 'Program')}
              {give(connect.args.map(word).join(' '), 'Arguments')}
            </>
          ) : (
            give(line)
          )}
          <p className="connect-note">Each app says its own name. What it adds in Gathr says who added it.</p>
        </>
      )}
    </Page>
  );
}
