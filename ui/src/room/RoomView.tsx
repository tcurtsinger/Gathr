import { type ReactNode, useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  type Agent,
  api,
  type Chat,
  type Failure,
  type Member,
  type Named,
  type PauseScope,
  type Pauses,
  type PickedName,
  type Project,
  type ReplyTarget,
} from '../api';
import { ChatMenu } from '../components/ChatMenu';
import { Composer, type Person } from '../components/Composer';
import { kept, type Picked } from '../components/mention';
import { FailureNotice } from '../components/Notice';
import { ProjectPick } from '../components/ProjectPick';
import { runOf } from '../pauses';
import { TopLine } from '../TopLine';
import { people as peopleOf, Room } from './Room';
import { type RoomState, useRoom } from './useRoom';

/** A message that never reached the service, kept by the app for Retry. */
interface Unsent {
  id: string;
  text: string;
  agent?: string;
  picks?: PickedName[];
  reply?: ReplyTarget;
  failure: Failure;
}

/** What the shell gives a chat's screen: the old chat's props, of which the room uses these. */
interface Props {
  chat: Chat;
  agents: Agent[];
  removed?: Named[];
  draft: string;
  picks?: Picked[];
  onDraft: (text: string, picks?: Picked[]) => void;
  openSettings: (page: 'engines' | 'diagnostics', turn?: string) => void;
  unsent: Unsent[];
  onUnsent: (update: (list: Unsent[]) => Unsent[]) => void;
  onNamed: () => void;
  projects: Project[];
  onProject: (project: string) => void;
  onNewProject: () => void;
  onStart: (text: string, agent?: string, picks?: PickedName[]) => void;
  notice?: ReactNode;
  pauses: Pauses;
  onPause: (action: 'pause' | 'resume' | 'stop', scope: PauseScope) => Promise<void>;
  onArchive?: (opener: HTMLElement | null) => void;
  onDelete?: (opener: HTMLElement | null) => void;
  onRestore?: () => void;
  restoreFailure?: Failure;
  onReference?: (ref: string) => void;
}

/** A chat in room mode: the middle of the window. */
export function RoomView(props: Props) {
  const { chat, agents, draft, picks = [], onDraft, unsent, onUnsent, onReference } = props;
  const { state, failure, reload, approve } = useRoom(chat.id);
  // A refused Stop, Pause or like: it shows above the message box, with its reference.
  const [refused, setRefused] = useState<Failure>();
  const known = useMemo(() => [...agents, ...(props.removed ?? [])], [agents, props.removed]);
  const fresh = !chat.id;
  const members = fresh ? (props.projects.find((p) => p.id === chat.project)?.members ?? []) : state.members;
  const team = useMemo(() => peopleOf(members, known), [members, known]);
  const who = nameable(members, agents, !chat.project);
  const to = recipients(picks, who, members, state);
  const running = Object.values(state.turns).some((t) => t.status === 'running');

  const send = (text: string, agent?: string, named?: PickedName[], again?: Unsent) => {
    if (fresh) {
      if (again) onUnsent((list) => list.filter((f) => f.id !== again.id));
      return props.onStart(text, agent, named);
    }
    const first = state.messages.length === 0;
    api.send(chat.id, text, agent, named).then(
      () => {
        if (again) onUnsent((list) => list.filter((f) => f.id !== again.id));
        if (first) props.onNamed();
      },
      (f: Failure) => {
        const lost = { id: again?.id ?? crypto.randomUUID(), text, agent, picks: named, failure: f };
        onUnsent((list) => [...list.filter((u) => u.id !== lost.id), lost]);
      },
    );
  };
  const sendDraft = () => {
    onDraft('', []);
    send(draft, to[0]?.id, kept(picks));
  };
  const attempt = (action: () => Promise<unknown>) =>
    action().then(
      () => setRefused(undefined),
      (f: Failure) => setRefused(f),
    );
  const thisChat: PauseScope = { scope: 'chat', target: chat.id };
  const stop = () => attempt(() => props.onPause('stop', thisChat));
  const run = runOf(props.pauses, chat.id);
  const reference = (ref: string) => (onReference ? onReference(ref) : props.openSettings('diagnostics'));
  const acts = {
    approve,
    reference,
    like: (message: string, give: boolean) =>
      attempt(() => (give ? api.like(chat.id, message) : api.unlike(chat.id, message))),
  };

  return (
    <>
      <TopLine>
        <div className="room-title">
          <h1>{chat.title}</h1>
          {!fresh && (
            <ChatMenu
              pause={{
                state: run.state,
                all: run.scope?.scope === 'app',
                onPause: () => attempt(() => props.onPause('pause', thisChat)),
                onResume: () => attempt(() => props.onPause('resume', run.scope ?? thisChat)),
              }}
              archived={!!chat.archived}
              onArchive={props.onArchive}
              onRestore={props.onRestore}
              onDelete={props.onDelete}
              className="head-button"
              align="start"
            />
          )}
          {chat.archived && <small>Archived</small>}
        </div>
      </TopLine>
      <main className="chat room" aria-label={chat.title}>
        {props.notice}
        <Room
          state={state}
          people={team}
          acts={acts}
          header={
            failure && (
              <FailureNotice
                failure={failure}
                onReference={onReference}
                action={
                  <Button type="button" variant="outline" onClick={reload}>
                    Retry
                  </Button>
                }
              />
            )
          }
        >
          {unsent.map((u) => (
            <FailureNotice
              key={u.id}
              failure={u.failure}
              onReference={onReference}
              links={<span className="room-unsent">{u.text}</span>}
              action={
                <Button type="button" variant="outline" onClick={() => send(u.text, u.agent, u.picks, u)}>
                  Retry
                </Button>
              }
            />
          ))}
          {refused && <FailureNotice failure={refused} onReference={onReference} />}
          {chat.archived ? (
            <>
              {props.restoreFailure && (
                <FailureNotice failure={props.restoreFailure} onReference={onReference} />
              )}
              <p className="room-archived">
                Archived
                <Button size="sm" variant="outline" onClick={props.onRestore}>
                  Restore
                </Button>
              </p>
            </>
          ) : (
            <>
              {fresh && (
                <ProjectPick
                  project={chat.project}
                  projects={props.projects}
                  onProject={props.onProject}
                  onNewProject={props.onNewProject}
                />
              )}
              <Composer
                draft={draft}
                picks={picks}
                onDraft={onDraft}
                onSend={sendDraft}
                onStop={running ? stop : undefined}
                hint="Message the team"
                people={who}
                to={to}
              />
            </>
          )}
        </Room>
      </main>
    </>
  );
}

/** Who "@" lists: the chat's team, then, in a chat with no project, every other agent, who joins by it. */
function nameable(members: Member[], agents: Agent[], open: boolean): Person[] {
  const team = members.map((m) => ({
    id: m.agent,
    name: m.name,
    meta: m.role === 'supervisor' ? 'Lead' : undefined,
  }));
  if (!open) return team;
  const others = agents.filter((a) => !members.some((m) => m.agent === a.id));
  return [...team, ...others.map((a) => ({ id: a.id, name: a.name, out: true }))];
}

/** Who gets the message: each agent it names, else the lead, else the agent that spoke last. */
function recipients(picks: Picked[], who: Person[], members: Member[], state: RoomState) {
  const named = [...picks].sort((a, b) => a.at - b.at);
  const each = named.filter((p, i) => named.findIndex((q) => q.id === p.id) === i);
  if (each.length)
    return each.map((p) => ({ id: p.id, name: p.name, joins: who.find((w) => w.id === p.id)?.out }));
  const lead = members.find((m) => m.role === 'supervisor');
  const last = Object.values(state.turns).findLast((t) => t.agent)?.agent;
  const one = lead?.agent ?? (members.length === 1 ? members[0].agent : last);
  const person = who.find((w) => w.id === one);
  return person ? [{ id: person.id, name: person.name }] : [];
}
