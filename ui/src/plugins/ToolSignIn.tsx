import { LoaderCircle } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, Failure } from '../api';
import { useCard } from '../components/Corner';
import { listen } from '../live';
import { OpenPage } from '../settings/SignIn';

type Asked = Parameters<typeof api.signInTool>[0];

/**
 * A tool's sign-in through the browser, from Sign in to its end: waiting for the browser, or failed, told with Retry.
 * `onDone` gets the tool that signed in; `onNeedsClient` the address for its admin, when it needs a Client ID first.
 */
export function useToolSignIn(onDone: (id: string) => void, onNeedsClient: (redirect: string) => void) {
  const [waiting, setWaiting] = useState<{ sign_in: string; url: string } | null>(null);
  const [failed, setFailed] = useState<Failure | null>(null);
  const [asked, setAsked] = useState<Asked | null>(null);
  useEffect(() => {
    if (!waiting) return;
    return listen((event) => {
      if (event.type !== 'tools' || event.sign_in !== waiting.sign_in) return;
      setWaiting(null);
      if (event.tool) onDone(event.tool);
      else setFailed(new Failure('Sign-in failed', event.why ?? '', event.ref ?? null));
    });
  }, [waiting, onDone]);
  const start = (tool: Asked) => {
    setFailed(null);
    setAsked(tool);
    api.signInTool(tool).then(
      (answer) => ('sign_in' in answer ? setWaiting(answer) : onNeedsClient(answer.redirect)),
      (f: Failure) => setFailed(new Failure('Sign-in failed', f.why ?? f.error, f.ref)),
    );
  };
  const cancel = () => {
    if (waiting) api.cancelToolSignIn(waiting.sign_in).catch(() => {});
    setWaiting(null);
    setFailed(null);
  };
  useCard(failed, { label: 'Retry', run: () => asked && start(asked) });
  return { waiting, failed, start, cancel };
}

/** Waiting for the browser: Open sign-in page opens it again. */
export function ToolSignInWaiting({ url, onCancel }: { url: string; onCancel: () => void }) {
  return (
    <>
      <span className="tool-checking muted" role="status">
        <LoaderCircle aria-hidden="true" className="tool-spinner" />
        Finish in your browser
      </span>
      <OpenPage url={url} small />
      <Button type="button" size="sm" variant="outline" onClick={onCancel}>
        Cancel
      </Button>
    </>
  );
}
