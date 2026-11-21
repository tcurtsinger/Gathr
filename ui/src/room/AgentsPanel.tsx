import { Terminal } from 'lucide-react';
import { Popover } from 'radix-ui';
import { useEffect, useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Background, type Failure, type Member, type Turn } from '../api';
import { useCard } from '../components/Corner';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { since } from '../components/marks';
import { AgentsMark } from '../Rail';
import type { People } from './Room';
import '../components/popover-panel.css';
import './agents-panel.css';

/**
 * The top line's Agents button and its panel: each agent of the chat with how full its conversation is, Compact,
 * and what its engine still runs in the background. The button's dot shows while any such command runs.
 */
export function AgentsPanel(props: {
  chat: string;
  members: Member[];
  background: Background[];
  turns: Record<string, Turn>;
  people: People;
}) {
  const { members, background } = props;
  const [open, setOpen] = useState(false);
  // The compaction last asked for here: if it fails, its turn says why, as its card in the chat would have.
  const [asked, setAsked] = useState<string>();
  const failed = asked ? props.turns[asked] : undefined;
  const [lost, why, ref] = [failed?.status === 'failed', failed?.why ?? null, failed?.ref ?? null];
  const told = useMemo(() => (lost ? { error: 'Compact failed', why, ref } : undefined), [lost, why, ref]);
  useCard(told);
  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <IconButton aria-label="Agents" className="head-button agents-button">
          <AgentsMark />
          {background.length > 0 && <span className="agents-dot" />}
        </IconButton>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          className="popover-panel agents-panel"
          side="bottom"
          align="end"
          sideOffset={8}
          collisionPadding={16}
          role="dialog"
          aria-label="Agents"
        >
          <h3>Agents</h3>
          {members.map((m) => (
            <Block
              key={m.agent}
              chat={props.chat}
              member={m}
              running={background.filter((b) => b.agent === m.agent)}
              turns={props.turns}
              people={props.people}
              onAsked={setAsked}
            />
          ))}
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}

/** Where the bar turns the attention blue: close to the point the conversation compacts at. */
const NEAR = 0.9;

/** One agent: its face and name, Compact, the bar "96K of 500K", and the commands it left running. */
function Block(props: {
  chat: string;
  member: Member;
  running: Background[];
  turns: Record<string, Turn>;
  people: People;
  onAsked: (turn: string) => void;
}) {
  const { chat, member, running, people } = props;
  const used = member.context?.used ?? null;
  const point = member.context?.compact_at ?? member.context?.window ?? null;
  const fill = used !== null && point ? Math.min(1, used / point) : 0;
  return (
    <section className="agents-block" aria-label={member.name}>
      <div className="agents-head">
        <Face agent={people.face(member.agent)} size={24} />
        <b>{member.name}</b>
        {member.role === 'supervisor' && <span className="room-tag">Lead</span>}
        <span className="room-grow" />
        {used !== null && (
          <Compact chat={chat} agent={member.agent} turns={props.turns} onAsked={props.onAsked} />
        )}
      </div>
      <div className="agents-meter">
        <span className="agents-track" data-near={fill >= NEAR || undefined}>
          {fill > 0 && <i style={{ width: `${fill * 100}%` }} />}
        </span>
        <span>{used === null ? 'New' : point ? `${tokens(used)} of ${tokens(point)}` : tokens(used)}</span>
      </div>
      {running.length > 0 && (
        <ul className="agents-commands">
          {running.map((b) => (
            <li key={`${b.started}${b.description}`}>
              <Terminal aria-hidden="true" />
              <span className="agents-what">{b.description}</span>
              <Since started={b.started} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** Compact, then Waiting while the agent's turn or commands run, Compacting while it compacts: its turn's status. */
function Compact(props: {
  chat: string;
  agent: string;
  turns: Record<string, Turn>;
  onAsked: (turn: string) => void;
}) {
  const { chat, agent, turns, onAsked } = props;
  const [asking, setAsking] = useState(false);
  const [refused, setRefused] = useState<Failure>();
  useCard(refused);
  const mine = Object.values(turns).filter((t) => t.agent === agent && t.opened === 'compaction');
  const under = mine.find(
    (t) => t.status === 'queued' || t.status === 'waiting_sign_in' || t.status === 'running',
  );
  if (under)
    return <span className="agents-word">{under.status === 'running' ? 'Compacting' : 'Waiting'}</span>;
  const compact = () => {
    setAsking(true);
    setRefused(undefined);
    api.compact(chat, agent).then(
      (a) => {
        onAsked(a.turn);
        setAsking(false);
      },
      (f: Failure) => {
        setRefused(f);
        setAsking(false);
      },
    );
  };
  return (
    <Button
      type="button"
      variant="outline"
      size="xs"
      className="agents-compact"
      disabled={asking}
      onClick={compact}
    >
      Compact
    </Button>
  );
}

/** How long a command has run, read again each 15 seconds: "18m". */
function Since({ started }: { started: string }) {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 15_000);
    return () => clearInterval(timer);
  }, []);
  return <span className="agents-since">{since(Math.max(now, Date.now()) - Date.parse(started))}</span>;
}

/** 950, 96K, 1.2M. */
function tokens(n: number) {
  if (n >= 1_000_000) return `${Number((n / 1_000_000).toFixed(1))}M`;
  return n >= 1000 ? `${Math.round(n / 1000)}K` : `${n}`;
}
