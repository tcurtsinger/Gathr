import { SquareTerminal } from 'lucide-react';

/** Connect an app's mark, in Plugins' sidebar: lucide's SquareTerminal, a stand-in until the owner gives theirs. */
export function ConnectMark({ className }: { className?: string }) {
  return <SquareTerminal aria-hidden="true" className={className} />;
}
