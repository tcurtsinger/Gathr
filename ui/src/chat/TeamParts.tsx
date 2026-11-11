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
export function GathrLine({ custom }: { custom: Custom }) {
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
        <TaskLine line={[who, assignment.what].filter(Boolean).join(' · ')} more={assignment.why} />
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

/** One line, cut with … when long; a click shows it whole, with what more there is (ux-issues 27). */
function TaskLine({ line, more }: { line: string; more?: string }) {
  return (
    <CollapsibleRoot>
      <CollapsibleTrigger className="task-line">{line}</CollapsibleTrigger>
      {more && <CollapsibleContent className="task-line-more">{more}</CollapsibleContent>}
    </CollapsibleRoot>
  );
}
