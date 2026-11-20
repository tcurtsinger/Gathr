import { MessageSquareText } from 'lucide-react';
import { ContextDisplay } from '@/components/assistant-ui/elements/context-display';
import { useContextDisplay } from '@/components/assistant-ui/utils/context-display-state';
import { getBarColor } from '@/components/assistant-ui/utils/context-usage';
import type { Context } from '../api';
import { Face } from '../components/Face';
import { IconButton } from '../components/IconButton';
import { CANT_READ, WORD } from '../components/marks';
import { ChatMenu } from './ChatMenu';
import { useMarks } from './faceMarks';
import type { Glance } from './glance';
import { began, roleName } from './team';

interface Props {
  glance: Glance;
  /** The task its running turn works on, said after Working. */
  task?: string;
  onMessage?: (agent: string) => void;
  onCompact?: (agent: string) => void;
  compaction?: 'queued' | 'running';
}

/**
 * An agent's panel header, two lines that never wrap (DESIGN.md "Agent panel"): the face with its mark, the name,
 * the role, Message and More; under them the state its mark shows, what, since when, and how full its context is.
 * The name and what give way first.
 */
export function AgentHead({ glance, task, onMessage, onCompact, compaction }: Props) {
  const { member, standing, since } = glance;
  const mark = useMarks()?.faces.get(member.agent);
  const what = standing === 'working' ? task : glance.about;
  return (
    <header className="agent-who">
      <Face agent={{ id: member.agent, name: member.name }} size={36} mark={mark} />
      <div className="agent-who-name">
        <h2>{member.name}</h2>
        <span>{roleName[member.role]}</span>
      </div>
      <span className="agent-who-actions">
        {onMessage && (
          <IconButton aria-label={`Message ${member.name}`} onClick={() => onMessage(member.agent)}>
            <MessageSquareText />
          </IconButton>
        )}
        {onCompact && (
          <ChatMenu
            label={`More for ${member.name}`}
            compact={{ asked: !!compaction, onCompact: () => onCompact(member.agent) }}
          />
        )}
      </span>
      <div className="agent-who-row">
        <span className="name-line-meta agent-state">
          <span
            className="name-line-end"
            data-bad={standing === 'cant' || undefined}
            data-you={standing === 'needs' || undefined}
          >
            {WORD[standing]}
          </span>
          {what && (
            <span className="name-line-task">
              <span className="name-line-what">{what}</span>
            </span>
          )}
          {glance.failing && <span>{CANT_READ}</span>}
          {since !== undefined && <span>since {began(since * 1000)}</span>}
        </span>
        <Fill context={member.context} compacting={compaction === 'running'} />
      </div>
    </header>
  );
}

/**
 * How full its context is (compaction plan, K1): the registry's context display, drawn as the panel's track and
 * percent, its numbers on hover. Until the conversation's first call is measured against a window, an empty track
 * says New; while a compaction asked here runs, Compacting.
 */
function Fill({ context, compacting }: { context?: Context | null; compacting: boolean }) {
  const { used, window } = context ?? {};
  if (compacting || used == null || !window)
    return (
      <span className="agent-fill">
        <span className="agent-fill-track" aria-hidden="true" />
        {compacting ? 'Compacting' : 'New'}
      </span>
    );
  return (
    <ContextDisplay.Root modelContextWindow={window} usage={{ totalTokens: used }}>
      <ContextDisplay.Trigger className="agent-fill" aria-label="Context usage">
        <Filled />
      </ContextDisplay.Trigger>
      <ContextDisplay.Content side="bottom" />
    </ContextDisplay.Root>
  );
}

function Filled() {
  const { percent } = useContextDisplay();
  return (
    <>
      <span className="agent-fill-track" aria-hidden="true">
        <span className={getBarColor(percent)} style={{ width: `${percent}%` }} />
      </span>
      {Math.round(percent)}%
    </>
  );
}
