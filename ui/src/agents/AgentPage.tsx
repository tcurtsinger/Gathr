import { ChevronLeft } from 'lucide-react';
import { type ReactNode, useEffect, useId, useState } from 'react';
import { Button } from '@/components/ui/button';
import { type Account, type Agent, type Approvals, api, type Failure, type Model } from '../api';
import { danger } from '../components/Confirm';
import { useCard } from '../components/Corner';
import { Dropdown } from '../components/Dropdown';
import { shuffled } from '../components/pictures';
import { DeleteConfirm } from '../DeleteConfirm';
import { report, timeToPaint } from '../report';
import type { ProposingAgent } from '../room/Proposal';
import { engineName } from '../settings/Accounts';
import { FaceFields } from './FaceFields';
import { choices, chosen, efforts, firstChoice } from './models';

interface Props {
  /** None for Add. */
  agent?: Agent;
  accounts: Account[];
  /** It is the assistant: its title says so. */
  assistant: boolean;
  saved: boolean;
  /** Change, on an agent the assistant proposed: Add's form filled in with it, and Save adds it from there. */
  proposal?: ProposingAgent | null;
  onBack: () => void;
  onSaved: (agent: Agent) => void;
  /** What the delete couldn't remove yet, if anything. */
  onDeleted: (left: Failure | null) => void;
}

/** Model is undefined until its account's models arrive: a new agent, or one moved to another account, takes the first. */
type Fields = {
  name: string;
  description?: string | null;
  account: string;
  model: string | null | undefined;
  effort: string | null;
  approvals: Approvals;
  shape: string | null;
  eyes: string | null;
  colour: string | null;
};

const modes: [Approvals, string, string][] = [
  ['ask', 'Ask', 'Ask before actions that need permission.'],
  ['edits', 'Edits', 'Allow file edits in the worktree. Ask before other actions.'],
  ['auto', 'Auto', 'The engine checks actions. Pushes and writes outside the worktree need approval.'],
  ['full', 'Full', 'Allow actions without asking.'],
];

/**
 * An agent's page, or Add's: its picture behind its box, then Name (Add's only), Description, Account (it implies
 * the engine), the account's models and their efforts, and Approvals, saved together with Save.
 */
export function AgentPage(props: Props) {
  const { agent, accounts, assistant, saved, proposal, onBack, onSaved, onDeleted } = props;
  // Only the fields edited here, over the agent as saved, so a save from elsewhere shows in
  // every field not edited here and Save never writes an untouched field back.
  const [edits, setEdits] = useState<Partial<Fields>>({});
  const edit = (fields: Partial<Fields>) => setEdits((e) => ({ ...e, ...fields }));
  // A new agent starts with three choices picked as Shuffle picks them, saved with it: its picture shows before it is.
  // A proposed one starts as proposed, with the face its card showed.
  const [blank] = useState<Fields>(() => ({
    name: '',
    account: accounts[0].id,
    model: undefined,
    effort: null,
    approvals: 'auto',
    ...shuffled(),
    ...proposal?.agent,
  }));
  const { name, description, account, model, effort, approvals, shape, eyes, colour }: Fields = {
    ...(agent ?? blank),
    ...edits,
  };
  const [busy, setBusy] = useState(false);
  /** Delete's button while its confirm is open: focus goes back to it on Cancel. */
  const [deleting, setDeleting] = useState<HTMLElement | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const models = useModels(account);
  useCard(failure);
  useCard(models.failure);
  const id = useId();

  const list = models.list ?? [];
  useEffect(() => {
    const loaded = models.list;
    // With no exact model listed, none is chosen and Save stays off: never the engine's default.
    const first = loaded && firstChoice(loaded);
    if (model === undefined && first) setEdits((e) => ({ ...e, model: first }));
  }, [model, models.list]);

  // A picture field goes with the save only when it changed, so Save never writes an untouched one back.
  const face = {
    ...(shape !== (agent?.shape ?? null) && { shape }),
    ...(eyes !== (agent?.eyes ?? null) && { eyes }),
    ...(colour !== (agent?.colour ?? null) && { colour }),
  };
  // The description too: sent only when it changed.
  const about = description?.trim() || null;
  const described = about !== (agent?.description ?? null) && { description: about };
  const draft = {
    name: name.trim(),
    account,
    model: model ?? null,
    effort,
    approvals,
    ...face,
    ...described,
  };
  const changed =
    !agent ||
    Object.keys(face).length > 0 ||
    !!described ||
    draft.account !== agent.account ||
    draft.model !== agent.model ||
    draft.effort !== agent.effort ||
    draft.approvals !== agent.approvals;
  const offered = efforts(list, draft.model);
  const exact = choices(list).map((c) => ({ value: c.id, label: c.label }));
  // Codex's sandbox keeps commands' writes in the worktree (0033).
  const claude = accounts.find((a) => a.id === account)?.engine === 'claude';
  const mode = modes.find(([m]) => m === approvals);

  const pickAccount = (id: string) => edit({ account: id, model: undefined, effort: null });
  const pickModel = (id: string) =>
    edit(effort && !efforts(list, id).includes(effort) ? { model: id, effort: null } : { model: id });
  const reset = () => {
    if (!agent) return onBack();
    setEdits({});
  };
  const submit = (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setFailure(null);
    const call = agent
      ? api.saveAgent({ id: agent.id, ...draft })
      : proposal
        ? api.addProposedAgent(proposal.message, draft).then((added) => added.agent)
        : api.addAgent(draft);
    call.then(
      (done) => {
        setBusy(false);
        setEdits({});
        onSaved(done);
      },
      (f: Failure) => {
        setBusy(false);
        setFailure(f);
      },
    );
  };

  return (
    <form className="agent-page" aria-labelledby={`${id}title`} onSubmit={submit}>
      <button type="button" className="screen-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Agents
      </button>
      <header className="screen-head">
        <FaceFields
          id={agent?.id ?? ''}
          name={name.trim()}
          picked={{ shape, eyes, colour }}
          onChange={edit}
        />
        <div className="screen-title">
          <h1 id={`${id}title`}>
            {agent ? agent.name : 'Add agent'}
            {assistant && <small className="agent-tag">Assistant</small>}
          </h1>
          {agent?.description && <p className="muted">{agent.description}</p>}
        </div>
        {agent && (
          <div className="screen-actions">
            <Button
              type="button"
              size="sm"
              variant="outline"
              className={danger}
              disabled={busy}
              onClick={(e) => setDeleting(e.currentTarget)}
            >
              Delete
            </Button>
          </div>
        )}
      </header>
      {agent && deleting && (
        <DeleteConfirm
          kind="agent"
          id={agent.id}
          name={agent.name}
          opener={deleting}
          onCancel={() => setDeleting(null)}
          onDeleted={onDeleted}
        />
      )}
      <div className="agent-rows">
        {/* An agent keeps the name it was added with: its folders carry it. */}
        {!agent && (
          <Row name="Name" id={`${id}name`}>
            <input
              id={`${id}name`}
              value={name}
              maxLength={80}
              autoComplete="off"
              spellCheck={false}
              // biome-ignore lint/a11y/noAutofocus: the page opens from Add, to type the name
              autoFocus
              onChange={(e) => edit({ name: e.target.value })}
            />
          </Row>
        )}
        <Row name="Description" id={`${id}about`}>
          <input
            id={`${id}about`}
            value={description ?? ''}
            maxLength={80}
            autoComplete="off"
            onChange={(e) => edit({ description: e.target.value })}
          />
        </Row>
        <Row name="Account" id={`${id}account`} line="Sets the engine.">
          <Dropdown
            id={`${id}account`}
            value={account}
            onChange={pickAccount}
            choices={accounts.map((a) => ({
              value: a.id,
              label: `${a.name} / ${engineName(a.engine)}${a.login === 'signed_out' ? ' / Signed out' : ''}`,
            }))}
          />
        </Row>
        <Row name="Model" id={`${id}model`} line="Stays until you change it.">
          {models.list === null && !models.failure ? (
            <p className="muted" role="status">
              Loading
            </p>
          ) : (
            <Dropdown
              id={`${id}model`}
              // Only an exact model is chosen in the list: an agent on a Latest or the engine default reads as the
              // model that is today, and picking that model saves it exactly.
              value={model ?? ''}
              shown={model === undefined ? undefined : chosen(list, model)}
              placeholder={exact.length > 0 ? undefined : 'No models'}
              onChange={pickModel}
              choices={exact}
            />
          )}
        </Row>
        <Row name="Effort" id={`${id}effort`}>
          <Dropdown
            id={`${id}effort`}
            value={effort ?? ''}
            onChange={(v) => edit({ effort: v || null })}
            choices={[
              { value: '', label: 'Engine default' },
              ...offered.map((level) => ({ value: level, label: level })),
              ...(effort && !offered.includes(effort)
                ? [{ value: effort, label: `${effort} / Unavailable` }]
                : []),
            ]}
          />
        </Row>
        <Row
          name="Approvals"
          id={`${id}approvals`}
          line={
            mode &&
            `${mode[2]}${claude && approvals === 'auto' ? ' Shell commands can run outside the worktree.' : ''}`
          }
        >
          <Dropdown
            id={`${id}approvals`}
            value={approvals}
            onChange={(v) => edit({ approvals: v as Approvals })}
            choices={modes.map(([mode, label]) => ({ value: mode, label }))}
          />
        </Row>
      </div>
      <footer className="agent-foot">
        <span className="muted" role="status">
          {saved && !changed ? 'Saved' : ''}
        </span>
        <Button
          type="button"
          size="sm"
          variant="ghost"
          onClick={reset}
          disabled={busy || (!!agent && !changed)}
        >
          Cancel
        </Button>
        <Button type="submit" size="sm" disabled={busy || !changed || !draft.name || model === undefined}>
          Save
        </Button>
      </footer>
    </form>
  );
}

/** One of the page's rows: the field's name with one line under it at the left, its control at the right. */
function Row({ name, id, line, children }: { name: string; id: string; line?: string; children: ReactNode }) {
  return (
    <div className="agent-row">
      <span className="agent-words">
        <label htmlFor={id}>{name}</label>
        {line && <span className="muted">{line}</span>}
      </span>
      <div className="agent-control">{children}</div>
    </div>
  );
}

/** The account's models as its engine lists them, asked again when the account changes. */
const NO_MODELS = { list: null as Model[] | null, failure: null as Failure | null };

function useModels(account: string) {
  const [state, setState] = useState({ of: account, ...NO_MODELS });
  useEffect(() => {
    let current = true;
    const started = performance.now();
    setState({ of: account, ...NO_MODELS });
    api.models(account).then(
      (list) => {
        if (!current) return;
        setState({ of: account, list, failure: null });
        timeToPaint('gathr.ui.models', started, { account, models: list.length });
      },
      (failure: Failure) => {
        if (!current) return;
        setState({ of: account, list: null, failure });
        report({ name: 'gathr.ui.models', error: failure.error, attrs: { account, ref: failure.ref } });
      },
    );
    return () => {
      current = false;
    };
  }, [account]);
  // Never the account before's list: a model is picked from it as soon as one is there.
  return state.of === account ? state : NO_MODELS;
}
