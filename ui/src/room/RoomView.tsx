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
  type Task,
} from '../api';
import { Composer, type Person } from '../components/Composer';
import { kept, type Picked } from '../components/mention';
import { FailureNotice } from '../components/Notice';
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

/**
 * What the shell gives a chat's screen: the same as the old chat's, so the switch is one line in the shell. The room
 * uses what it needs and leaves the rest (the workspace, tasks, plans, questions, search hits) to the steps that
 * build them.
 */
interface Props {
  chat: Chat;
  agents: Agent[];
  removed?: Named[];
  draft: string;
  picks?: Picked[];
  onDraft: (text: string, picks?: Picked[]) => void;
  replyDraft?: (topic: string) => { text: string; picks: Picked[] } | undefined;
  onReplyDraft?: (topic: string, text: string, picks: Picked[]) => void;
  openSettings: (page: 'engines' | 'diagnostics', turn?: string) => void;
  unsent: Unsent[];
  onUnsent: (update: (list: Unsent[]) => Unsent[]) => void;
  onNamed: () => void;
  projects: Project[];
  onProject: (project: string) => void;
  onNewProject: () => void;
  onStart: (text: string, agent?: string, picks?: PickedName[]) => void;
  headerEnd?: ReactNode;
  notice?: ReactNode;
  pauses: Pauses;
  onPause: (action: 'pause' | 'resume' | 'stop', scope: PauseScope) => Promise<void>;
  onArchive?: (opener: HTMLElement | null) => void;
  onDelete?: (opener: HTMLElement | null) => void;
  onRestore?: () => void;
  restoreFailure?: Failure;
  onReference?: (ref: string) => void;
  onOpenTask?: (task: number) => void;
  agentOpen?: { agent: string; shown: boolean };
  onOpenAgent?: (agent: string) => void;
  onOpenPlan?: (plan: number, version: number) => void;
  workspace?: (
    agent: { id: string; name: string; panel: ReactNode } | undefined,
    work: { tasks: Task[]; members: Member[] },
  ) => ReactNode;
  found?: { message?: string; at: number };
  onFound?: () => void;
  answers?: unknown;
}

/** A chat in room mode: the middle of the window. */
export function RoomView(props: Props) {
  const { chat, agents, draft, picks = [], onDraft, unsent, onUnsent, onReference } = props;
  const { state, failure, reload } = useRoom(chat.id);
  const [stopFailure, setStopFailure] = useState<Failure>();
  const known = useMemo(() => [...agents, ...(props.removed ?? [])], [agents, props.removed]);
  const fresh = !chat.id;
  const members = fresh ? (props.projects.find((p) => p.id === chat.project)?.members ?? []) : state.members;
  const team = useMemo(() => peopleOf(members, known), [members, known]);
  const who = nameable(members, agents, !chat.project);
  const to = recipients(picks, who, members, state);
  const running = Object.values(state.turns).some((t) => t.status === 'running');

  const send = (text: string, agent?: string, named?: PickedName[], again?: Unsent) => {
    if (fresh) return props.onStart(text, agent, named);
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
  const stop = () =>
    props.onPause('stop', { scope: 'chat', target: chat.id }).then(
      () => setStopFailure(undefined),
      (f: Failure) => setStopFailure(f),
    );

  return (
    <>
      <TopLine>
        <div className="room-title">
          <h1>{chat.title}</h1>
        </div>
        {props.headerEnd}
      </TopLine>
      <main className="chat room" aria-label={chat.title}>
        {props.notice}
        <Room
          state={state}
          people={team}
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
          {stopFailure && <FailureNotice failure={stopFailure} onReference={onReference} />}
          {chat.archived ? (
            <p className="room-archived">
              Archived
              <Button size="sm" variant="outline" onClick={props.onRestore}>
                Restore
              </Button>
            </p>
          ) : (
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
