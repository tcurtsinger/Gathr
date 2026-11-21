import { ChevronLeft, Plug } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type ToolAction, type WebTool } from '../api';
import { danger } from '../components/Confirm';
import { Switch } from '../components/Switch';
import { ChangeClientId, given } from './ClientId';
import { ToolSignInWaiting, useToolSignIn } from './ToolSignIn';
import { stateOf } from './tool';

interface Props {
  /** As the list has it now. */
  tool: WebTool;
  onBack: () => void;
  onFailure: (failure: Failure) => void;
  onEnable: (tool: WebTool, on: boolean) => void;
  onChangeKey: (tool: WebTool, opener: HTMLElement | null) => void;
  onRemove: (tool: WebTool, opener: HTMLElement | null) => void;
}

/**
 * A tool's page: its link and state, Change key (Sign in again for one signed in through the browser) and Remove,
 * Enabled, its Client ID with Change for one that signs in with one, and its actions asked of it now.
 */
export function ToolPage({ tool, onBack, onFailure, onEnable, onChangeKey, onRemove }: Props) {
  const [asked, setAsked] = useState<(WebTool & { actions: ToolAction[] | null; redirect?: string }) | null>(
    null,
  );
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
            <code>{tool.link}</code>
            {asked && (
              <span className="tool-state" data-tone={state.tone}>
                <i className="plugins-dot" data-tone={state.tone ?? 'ok'} aria-hidden="true" />
                {state.words}
              </span>
            )}
          </p>
        </div>
        <div className="screen-actions">
          {tool.signed_in ? (
            <Button
              type="button"
              size="sm"
              variant="outline"
              disabled={!!signIn.waiting}
              onClick={() => signIn.start({ tool: tool.id })}
            >
              Sign in again
            </Button>
          ) : (
            <Button
              ref={changeKey}
              type="button"
              size="sm"
              variant="outline"
              onClick={() => onChangeKey(tool, changeKey.current)}
            >
              Change key
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
            When it’s disabled, no agent can use it. It stays installed.
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
