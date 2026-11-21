import { type ReactNode, useMemo, useRef, useState } from 'react';
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
import { WorkspaceToggle } from '../components/workspace';
import { runOf } from '../pauses';
import { TopLine } from '../TopLine';
import { Questions } from './Question';
import { type Found, people as peopleOf, Room } from './Room';
import { RoomSidebar } from './Sidebar';
import { type RoomState, useRoom } from './useRoom';

/** A message that never reached the service, kept by the app for Retry. */
export interface Unsent {
  id: string;
  text: string;
  agent?: string;
  picks?: PickedName[];
  reply?: ReplyTarget;
  files?: readonly File[];
  failure: Failure;
}

/** What the shell gives a chat's screen. */
interface Props {
  chat: Chat;
  agents: Agent[];
  removed?: Named[];
  draft: string;
  picks?: Picked[];
  onDraft: (text: string, picks?: Picked[]) => void;
  /** The files added to the draft, kept until Send. */
  files?: File[];
  onFiles: (files: File[]) => void;
  openSettings: (page: 'engines' | 'diagnostics', turn?: string) => void;
  unsent: Unsent[];
  onUnsent: (update: (list: Unsent[]) => Unsent[]) => void;
  onNamed: () => void;
  projects: Project[];
  onProject: (project: string) => void;
  onNewProject: () => void;
  onStart: (text: string, agent?: string, picks?: PickedName[], files?: readonly File[]) => void;
  notice?: ReactNode;
  pauses: Pauses;
  onPause: (action: 'pause' | 'resume' | 'stop', scope: PauseScope) => Promise<void>;
  onArchive?: (opener: HTMLElement | null) => void;
  onDelete?: (opener: HTMLElement | null) => void;
  onRestore?: () => void;
  restoreFailure?: Failure;
  onReference?: (ref: string) => void;
  /** The message a search hit opened the chat at. */
  found?: Found;
  /** Whether the sidebar is open, as the person left it in this chat; unset, it opens with more than one agent. */
  sidebar?: boolean;
  onSidebar: (open: boolean) => void;
  onFailure: (f: Failure) => void;
}

/** A chat: the middle of the window, and its sidebar. */
export function RoomView(props: Props) {
  const { chat, agents, draft, picks = [], onDraft, unsent, onUnsent, onReference } = props;
  const { state, failure, reload, approve, answer } = useRoom(chat.id);
  // A refused Stop, Pause or like: it shows above the message box, with its reference.
  const [refused, setRefused] = useState<Failure>();
  // The questions panel: closed, open, or open on one question.
  const [asking, setAsking] = useState<number | null>();
  const known = useMemo(() => [...agents, ...(props.removed ?? [])], [agents, props.removed]);
  const fresh = !chat.id;
  const members = fresh ? (props.projects.find((p) => p.id === chat.project)?.members ?? []) : state.members;
  const team = useMemo(() => peopleOf(members, known), [members, known]);
  const who = nameable(members, agents, !chat.project);
  const to = recipients(picks, who, members, state);
  const running = Object.values(state.turns).some((t) => t.status === 'running');
  const sidebar = !fresh && (props.sidebar ?? state.members.length > 1);

  const send = (
    text: string,
    agent?: string,
    named?: PickedName[],
    files?: readonly File[],
    again?: Unsent,
  ) => {
    if (fresh) {
      if (again) onUnsent((list) => list.filter((f) => f.id !== again.id));
      return props.onStart(text, agent, named, files);
    }
    const first = state.messages.length === 0;
    api.send(chat.id, text, agent, named, undefined, files).then(
      () => {
        if (again) onUnsent((list) => list.filter((f) => f.id !== again.id));
        if (first) props.onNamed();
      },
      (f: Failure) => {
        const lost = { id: again?.id ?? crypto.randomUUID(), text, agent, picks: named, files, failure: f };
        onUnsent((list) => [...list.filter((u) => u.id !== lost.id), lost]);
      },
    );
  };
  const sendDraft = () => {
    onDraft('', []);
    props.onFiles([]);
    send(draft, to[0]?.id, kept(picks), props.files);
  };
  const column = useRef<HTMLElement>(null);
  const attempt = (action: () => Promise<unknown>) =>
    action().then(
      () => setRefused(undefined),
      (f: Failure) => setRefused(f),
    );
  const thisChat: PauseScope = { scope: 'chat', target: chat.id };
  const stop = () => attempt(() => props.onPause('stop', thisChat));
  const run = runOf(props.pauses, chat.id);
  const resume = () => attempt(() => props.onPause('resume', run.scope ?? thisChat));
  const reference = (ref: string) => (onReference ? onReference(ref) : props.openSettings('diagnostics'));
  const acts = {
    approve,
    ask: setAsking,
    reference,
    like: chat.archived
      ? undefined
      : (message: string, give: boolean) =>
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
                onResume: resume,
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
        {!fresh && (
          <div className="room-actions">
            <Questions
              questions={state.questions}
              people={team}
              at={asking}
              onOpen={setAsking}
              onAnswer={answer}
              onReference={reference}
            />
            <WorkspaceToggle open={sidebar} onToggle={() => props.onSidebar(!sidebar)} />
          </div>
        )}
      </TopLine>
      <main className="chat room" aria-label={chat.title} ref={column}>
        {props.notice}
        <Room
          chat={chat.id}
          state={state}
          people={team}
          acts={acts}
          found={props.found}
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
              links={<span className="room-unsent">{u.text || u.files?.map((f) => f.name).join(', ')}</span>}
              action={
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => send(u.text, u.agent, u.picks, u.files, u)}
                >
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
              {!fresh && run.state !== 'running' && (
                <p className="room-run" role="status">
                  {run.state === 'pausing' ? 'Pausing…' : run.state === 'paused' ? 'Paused' : 'Stopped'}
                  <Button size="sm" variant="outline" onClick={resume}>
                    {run.state === 'stopped' ? 'Restart' : 'Resume'}
                  </Button>
                </p>
              )}
              <Composer
                draft={draft}
                picks={picks}
                onDraft={onDraft}
                files={props.files}
                onFiles={props.onFiles}
                dropOn={column}
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
      {sidebar && <RoomSidebar chat={chat.id} onFailure={props.onFailure} />}
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
