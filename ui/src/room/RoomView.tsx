import { useMemo, useRef, useState } from 'react';
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
  type Settings,
} from '../api';
import { ChatMenu } from '../components/ChatMenu';
import { Composer, type Person } from '../components/Composer';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { FileTiles, LocalTile } from '../components/FileTile';
import { kept, type Picked } from '../components/mention';
import { ProjectPick } from '../components/ProjectPick';
import { WorkspaceToggle } from '../components/workspace';
import { runOf } from '../pauses';
import { TopLine } from '../TopLine';
import { AgentsPanel } from './AgentsPanel';
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
  /** The person's name and assistant, for a new chat's greeting. */
  you: Pick<Settings, 'person_name' | 'assistant'>;
  draft: string;
  picks?: Picked[];
  onDraft: (text: string, picks?: Picked[]) => void;
  /** The files added to the draft, kept until Send. */
  files?: File[];
  onFiles: (files: File[]) => void;
  unsent: Unsent[];
  onUnsent: (update: (list: Unsent[]) => Unsent[]) => void;
  onNamed: () => void;
  projects: Project[];
  onProject: (project: string) => void;
  onNewProject: () => void;
  onStart: (text: string, agent?: string, picks?: PickedName[], files?: readonly File[]) => void;
  pauses: Pauses;
  onPause: (action: 'pause' | 'resume' | 'stop', scope: PauseScope) => Promise<void>;
  onArchive?: (opener: HTMLElement | null) => void;
  onDelete?: (opener: HTMLElement | null) => void;
  onRestore?: () => void;
  /** The message a search hit opened the chat at. */
  found?: Found;
  /** Whether the sidebar is open, as the person left it in this chat; unset, it opens with more than one agent. */
  sidebar?: boolean;
  onSidebar: (open: boolean) => void;
}

/** A chat: the middle of the window, and its sidebar. */
export function RoomView(props: Props) {
  const { chat, agents, draft, picks = [], onDraft, unsent, onUnsent } = props;
  const { state, failure, reload, approve, answer } = useRoom(chat.id);
  useCard(failure, { label: 'Retry', run: reload });
  // A refused Stop, Pause or like.
  const [refused, setRefused] = useState<Failure>();
  useCard(refused);
  // The questions panel: closed, open, or open on one question.
  const [asking, setAsking] = useState<number | null>();
  const known = useMemo(() => [...agents, ...(props.removed ?? [])], [agents, props.removed]);
  const fresh = !chat.id;
  // A new chat in a project asks what to build there, under its lead's face; with no project, the greeting.
  const project = props.projects.find((p) => p.id === chat.project)?.name;
  const members = fresh ? (props.projects.find((p) => p.id === chat.project)?.members ?? []) : state.members;
  const team = useMemo(() => peopleOf(members, known), [members, known]);
  // A new chat with no project: a message that names nobody goes to the assistant.
  const assistant = fresh && !chat.project ? agents.find((a) => a.id === props.you.assistant) : undefined;
  const who = nameable(members, agents, !chat.project, assistant);
  const to = recipients(picks, who, members, state, assistant);
  // Who a new chat's message goes to, by itself: the assistant, or the project's lead.
  const ahead = fresh ? recipients([], who, members, state, assistant)[0] : undefined;
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
  const acts = {
    approve,
    ask: setAsking,
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
            <AgentsPanel
              chat={chat.id}
              members={state.members}
              background={state.background}
              turns={state.turns}
              people={team}
            />
            <Questions
              questions={state.questions}
              people={team}
              at={asking}
              onOpen={setAsking}
              onAnswer={answer}
            />
            <WorkspaceToggle open={sidebar} onToggle={() => props.onSidebar(!sidebar)} />
          </div>
        )}
      </TopLine>
      <main className="chat room" aria-label={chat.title} ref={column}>
        <Room
          chat={chat.id}
          state={state}
          people={team}
          acts={acts}
          found={props.found}
          header={
            fresh && (
              <div className="room-greeting">
                {ahead && <Face agent={{ id: ahead.id, name: ahead.name }} size={56} />}
                <p>
                  {project
                    ? `What should we build in ${project}?`
                    : greeting(new Date().getHours(), props.you.person_name)}
                </p>
              </div>
            )
          }
          after={unsent.map((u) => (
            <NotSent key={u.id} unsent={u} onRetry={() => send(u.text, u.agent, u.picks, u.files, u)} />
          ))}
        >
          {chat.archived ? (
            <p className="room-archived">
              Archived
              <Button size="sm" variant="outline" onClick={props.onRestore}>
                Restore
              </Button>
            </p>
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
                hint={ahead ? `Message ${ahead.name}` : 'Message the team'}
                people={who}
                to={to}
              />
            </>
          )}
        </Room>
      </main>
      {sidebar && <RoomSidebar chat={chat.id} />}
    </>
  );
}

/** A message the service didn't take: the person's own bubble, dimmed where it would be, with Retry; told too. */
function NotSent({ unsent: u, onRetry }: { unsent: Unsent; onRetry: () => void }) {
  useCard(u.failure, { label: 'Retry', run: onRetry });
  const tiles = !!u.files?.length && (
    <FileTiles>
      {u.files.map((f, i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: the same file may be attached twice
        <LocalTile key={i} file={f} />
      ))}
    </FileTiles>
  );
  return (
    <div className="room-item room-unsent">
      {u.text ? (
        <div className="room-you">
          <span className="room-words">{u.text}</span>
          {tiles}
        </div>
      ) : (
        <div className="room-you-files">{tiles}</div>
      )}
      <Button type="button" variant="ghost" size="xs" onClick={onRetry}>
        Retry
      </Button>
    </div>
  );
}

/**
 * Who "@" lists: the chat's team, then, in a chat with no project, every other agent, who joins by it; the assistant
 * first, marked, in a new chat with no project.
 */
function nameable(members: Member[], agents: Agent[], open: boolean, assistant?: Agent): Person[] {
  const about = (id: string) => agents.find((a) => a.id === id)?.description ?? undefined;
  const team = members.map((m) => ({
    id: m.agent,
    name: m.name,
    about: about(m.agent),
    meta: m.role === 'supervisor' ? 'Lead' : undefined,
  }));
  if (!open) return team;
  const others = agents.filter((a) => !members.some((m) => m.agent === a.id));
  const ordered = assistant ? [assistant, ...others.filter((a) => a !== assistant)] : others;
  return [
    ...team,
    ...ordered.map((a) => ({
      id: a.id,
      name: a.name,
      about: about(a.id),
      meta: a === assistant ? 'Assistant' : undefined,
      out: true,
    })),
  ];
}

/** Who gets the message: each agent it names, else the lead, else the agent that spoke last, else the assistant. */
function recipients(picks: Picked[], who: Person[], members: Member[], state: RoomState, assistant?: Agent) {
  const named = [...picks].sort((a, b) => a.at - b.at);
  const each = named.filter((p, i) => named.findIndex((q) => q.id === p.id) === i);
  if (each.length)
    return each.map((p) => ({ id: p.id, name: p.name, joins: who.find((w) => w.id === p.id)?.out }));
  const lead = members.find((m) => m.role === 'supervisor');
  const last = Object.values(state.turns).findLast((t) => t.agent)?.agent;
  const one = lead?.agent ?? (members.length === 1 ? members[0].agent : (last ?? assistant?.id));
  const person = who.find((w) => w.id === one);
  return person ? [{ id: person.id, name: person.name }] : [];
}

/** A new chat's line, by the computer's clock: "Good morning, Sam", "Still up?". */
function greeting(hour: number, name?: string | null) {
  if (hour < 5) return name ? `Still up, ${name}?` : 'Still up?';
  const part = hour < 12 ? 'morning' : hour < 17 ? 'afternoon' : 'evening';
  return name ? `Good ${part}, ${name}` : `Good ${part}`;
}
