import { ChevronLeft, Plug } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type PcRuns, type ToolAction, type WebTool } from '../api';
import { danger } from '../components/Confirm';
import { Switch } from '../components/Switch';
import { ChangeClientId, given } from './ClientId';
import type { ToolNext } from './Plugins';
import { Runs } from './Runs';
import { ToolSignInWaiting, useToolSignIn } from './ToolSignIn';
import { stateOf } from './tool';

interface Props {
  /** As the list has it now. */
  tool: WebTool;
  /** Where the page goes on to once it's asked of the tool: a chat's line for it opened it so. */
  next?: ToolNext;
  onBack: () => void;
  onFailure: (failure: Failure) => void;
  onEnable: (tool: WebTool, on: boolean) => void;
  onChangeKey: (tool: WebTool, opener: HTMLElement | null) => void;
  onRemove: (tool: WebTool, opener: HTMLElement | null) => void;
  /** Change, for a tool on this PC: what it runs, as a JSON block with each variable empty. */
  onChangeRuns: (tool: WebTool, text: string) => void;
}

type Asked = WebTool & { actions: ToolAction[] | null; redirect?: string } & Partial<PcRuns> & {
    printed?: string | null;
  };

/** What a tool on this PC runs, as the JSON block its instructions would give, each variable with no value. */
function asJson(name: string, runs: PcRuns): string {
  const entry: Record<string, unknown> = { command: runs.typed };
  if (runs.args.length) entry.args = runs.args;
  if (runs.folder) entry.cwd = runs.folder;
  if (runs.variables.length) entry.env = Object.fromEntries(runs.variables.map((v) => [v, '']));
  return JSON.stringify({ mcpServers: { [name]: entry } }, null, 2);
}

/**
 * A tool's page: its link and state, Change key (Sign in again for one signed in through the browser) and Remove,
 * Enabled, its Client ID with Change for one that signs in with one, and its actions asked of it now. A tool on this
 * PC has what Gathr runs with Change, Try again when it didn't start, and then what it printed.
 */
export function ToolPage({
  tool,
  next,
  onBack,
  onFailure,
  onEnable,
  onChangeKey,
  onRemove,
  onChangeRuns,
}: Props) {
  const [asked, setAsked] = useState<Asked | null>(null);
  const [trying, setTrying] = useState(false);
  const pc = tool.kind === 'pc';
  const changeKey = useRef<HTMLButtonElement>(null);
  const changeClient = useRef<HTMLButtonElement>(null);
  const remove = useRef<HTMLButtonElement>(null);
  /** Change Client ID open, with the address the admin's app must have. */
  const [changing, setChanging] = useState<{ redirect: string; opener: HTMLElement | null } | null>(null);
  // Signed in again: the list's `tools` change asks the page again.
  const signIn = useToolSignIn(
    useCallback(() => {}, []),
    useCallback((redirect: string) => setChanging({ redirect, opener: null }), []),
  );
  // Asked again each time the list changes: a new key or a tool back up comes with one.
  const listed = JSON.stringify(tool);
  useEffect(() => {
    if (listed) api.tool(tool.id).then(setAsked, onFailure);
  }, [tool.id, listed, onFailure]);
  const shown = asked ? { ...tool, state: asked.state } : tool;
  const state = stateOf({ ...shown, enabled: true });
  const actions = asked?.state === 'connected' ? asked.actions : null;
  const runs: PcRuns | null = pc && asked?.typed !== undefined ? (asked as PcRuns) : null;
  // A tool an agent saved without its key or sign-in, or with a value left empty: the person gives it here.
  const signsIn = tool.signed_in || asked?.state === 'sign_in';
  const keyed = asked?.state === 'needs_key' ? 'Add key' : 'Change key';
  const went = useRef(false);
  useEffect(() => {
    if (!next || !asked || went.current) return;
    went.current = true;
    if (next === 'key') onChangeKey(tool, changeKey.current);
    if (next === 'sign-in') signIn.start({ tool: tool.id });
    if (next === 'change' && runs) onChangeRuns(tool, asJson(tool.name, runs));
  }, [next, asked, tool, runs, onChangeKey, onChangeRuns, signIn]);
  const tryAgain = () => {
    setTrying(true);
    api
      .startTool(tool.id)
      .then(() => api.tool(tool.id).then(setAsked), onFailure)
      .finally(() => setTrying(false));
  };
  return (
    <article className="skill-page">
      <button type="button" className="screen-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Plugins
      </button>
      <header className="screen-head">
        <span className="plugin-tile" aria-hidden="true">
          <Plug />
        </span>
        <div className="screen-title">
          <h1>{tool.name}</h1>
          <p className="tool-line">
            {pc ? <span className="muted">On this PC</span> : <code>{tool.link}</code>}
            {asked && (!pc || asked.state) && (
              <span className="tool-state" data-tone={state.tone}>
                <i className="plugins-dot" data-tone={state.tone ?? 'ok'} aria-hidden="true" />
                {state.words}
              </span>
            )}
          </p>
        </div>
        <div className="screen-actions">
          {pc ? (
            asked?.state === 'not_started' && (
              <Button type="button" size="sm" variant="outline" disabled={trying} onClick={tryAgain}>
                Try again
              </Button>
            )
          ) : signsIn ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={!!signIn.waiting}
              onClick={() => signIn.start({ tool: tool.id })}
            >
              {tool.signed_in ? 'Sign in again' : 'Sign in'}
            </Button>
          ) : (
            <Button
              ref={changeKey}
              type="button"
              size="sm"
              variant="outline"
              onClick={() => onChangeKey(tool, changeKey.current)}
            >
              {keyed}
            </Button>
          )}
          <Button
            ref={remove}
            type="button"
            size="sm"
            variant="outline"
            className={danger}
            onClick={() => onRemove(tool, remove.current)}
          >
            Remove
          </Button>
        </div>
      </header>
      {signIn.waiting && (
        <footer className="agent-actions tool-sign-in">
          <ToolSignInWaiting url={signIn.waiting.url} onCancel={signIn.cancel} />
        </footer>
      )}
      <div className="skill-enabled">
        <span className="skill-enabled-words">
          <span id="tool-enabled">Enabled</span>
          <span className="muted" id="tool-enabled-why">
            {pc
              ? 'While it’s enabled, Gathr keeps it running for every agent. Disabled, it’s stopped.'
              : 'When it’s disabled, no agent can use it. It stays installed.'}
          </span>
        </span>
        <Switch
          on={tool.enabled}
          labelledBy="tool-enabled"
          describedBy="tool-enabled-why"
          onChange={(on) => onEnable(tool, on)}
        />
      </div>
      {tool.client_id && (
        <div className="tool-client">
          <span className="skill-enabled-words">
            <span>Client ID</span>
            <code className="muted">{tool.client_id}</code>
          </span>
          <Button
            ref={changeClient}
            type="button"
            size="sm"
            variant="outline"
            disabled={!asked?.redirect}
            onClick={() =>
              asked?.redirect && setChanging({ redirect: asked.redirect, opener: changeClient.current })
            }
          >
            Change
          </Button>
        </div>
      )}
      {runs && (
        <section className="tool-block">
          <div className="tool-block-head">
            <h2>What Gathr runs</h2>
            <Button
              type="button"
              size="sm"
              variant="outline"
              onClick={() => onChangeRuns(tool, asJson(tool.name, runs))}
            >
              Change
            </Button>
          </div>
          <Runs runs={runs} />
        </section>
      )}
      {actions && (
        <section className="tool-actions">
          <h2>
            Actions <span className="nav-count">{actions.length}</span>
          </h2>
          <p className="muted">
            What it lets an agent do. Each agent asks first or not as its Approvals say.
          </p>
          <ul>
            {actions.map((a) => (
              <li key={a.name}>
                <strong className="tool-action-name">{a.name}</strong>
                {a.description && <span className="muted">{a.description}</span>}
              </li>
            ))}
          </ul>
        </section>
      )}
      {asked?.state === 'not_started' && asked.printed && (
        <section className="tool-block">
          <div className="tool-block-head">
            <h2>What it printed</h2>
          </div>
          <pre className="tool-printed">{asked.printed}</pre>
        </section>
      )}
      {changing && (
        <ChangeClientId
          redirect={changing.redirect}
          opener={changing.opener}
          onClose={() => setChanging(null)}
          onSignIn={(client) => {
            setChanging(null);
            signIn.start({ tool: tool.id, ...given(client) });
          }}
        />
      )}
    </article>
  );
}
