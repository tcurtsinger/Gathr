import { MessagePrimitive } from '@assistant-ui/react';
import type { Custom } from './messages';
import './team.css';

/** What Gathr told an agent: a member's task above its work, else a wake-up in Gathr's own voice. */
export function GathrLine({ custom }: { custom: Custom }) {
  const { assignment } = custom;
  if (assignment) {
    const who = assignment.from && assignment.to ? `${assignment.from} to ${assignment.to}` : assignment.to;
    return (
      <MessagePrimitive.Root className="assignment">
        {[who, assignment.what].filter(Boolean).join(' · ')}
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
