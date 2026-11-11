import { MessagePrimitive } from '@assistant-ui/react';
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from '@/components/assistant-ui/elements/collapsible-root';
import type { Custom } from './messages';
import './team.css';

/**
 * What Gathr told an agent: the person's answers it heard, a member's task above its work, else a
 * wake-up in Gathr's own voice.
 */
export function GathrLine({ custom, onOpenTask }: { custom: Custom; onOpenTask?: (task: number) => void }) {
  const { assignment, answered } = custom;
  if (answered)
    return (
      <MessagePrimitive.Root className="answered">
        {answered.map(({ line, asked }) => (
          <TaskLine key={line} line={line} more={asked} />
        ))}
      </MessagePrimitive.Root>
    );
  if (assignment) {
    const who = assignment.from && assignment.to ? `${assignment.from} to ${assignment.to}` : assignment.to;
    return (
      <MessagePrimitive.Root className="assignment">
        <TaskLine
          line={[who, assignment.what].filter(Boolean).join(' · ')}
          more={assignment.why}
          open={onOpenTask && (() => onOpenTask(assignment.number))}
        />
      </MessagePrimitive.Root>
    );
  }
  return (
    <MessagePrimitive.Root className="gathr-line">
      <span className="gathr-name">Gathr</span>
      <span className="gathr-text">{custom.gathr}</span>
    </MessagePrimitive.Root>
  );
}

/**
 * One line, cut with … when long; a click shows it whole, with what more there is (ux-issues 27) and,
 * for a hand-off, Open, which shows the task in the Tasks tab (room task 70).
 */
function TaskLine({ line, more, open }: { line: string; more?: string; open?: () => void }) {
  return (
    <CollapsibleRoot>
      <CollapsibleTrigger className="task-line">{line}</CollapsibleTrigger>
      {(more || open) && (
        <CollapsibleContent className="task-line-more">
          {more && <span>{more}</span>}
          {open && (
            <button type="button" className="link task-open" onClick={open}>
              Open task
            </button>
          )}
        </CollapsibleContent>
      )}
    </CollapsibleRoot>
  );
}
