import { LoaderCircle } from 'lucide-react';
import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { Notice } from '../components/Notice';
import { listen } from '../live';
import { OpenPage } from '../settings/SignIn';

type Asked = Parameters<typeof api.signInTool>[0];

/**
 * A tool's sign-in through the browser, from Sign in to its end: waiting for the browser, or failed with why.
 * `onDone` gets the tool that signed in; `onNeedsClient` the address for its admin, when it needs a Client ID first.
 */
export function useToolSignIn(onDone: (id: string) => void, onNeedsClient: (redirect: string) => void) {
  const [waiting, setWaiting] = useState<{ sign_in: string; url: string } | null>(null);
  const [failed, setFailed] = useState<{ why: string; ref?: string } | null>(null);
  const [asked, setAsked] = useState<Asked | null>(null);
  useEffect(() => {
    if (!waiting) return;
    return listen((event) => {
      if (event.type !== 'tools' || event.sign_in !== waiting.sign_in) return;
      setWaiting(null);
      if (event.tool) onDone(event.tool);
      else setFailed({ why: event.why ?? '', ref: event.ref });
    });
  }, [waiting, onDone]);
  const start = (tool: Asked) => {
    setFailed(null);
    setAsked(tool);
    api.signInTool(tool).then(
      (answer) => ('sign_in' in answer ? setWaiting(answer) : onNeedsClient(answer.redirect)),
      (f: Failure) => setFailed({ why: f.why ?? f.error, ref: f.ref ?? undefined }),
    );
  };
  const cancel = () => {
    if (waiting) api.cancelToolSignIn(waiting.sign_in).catch(() => {});
    setWaiting(null);
    setFailed(null);
  };
  const retry = () => asked && start(asked);
  return { waiting, failed, start, retry, cancel };
}

/** Waiting for the browser: Open sign-in page opens it again. */
export function ToolSignInWaiting({ url, onCancel }: { url: string; onCancel: () => void }) {
  return (
    <>
      <span className="tool-checking muted" role="status">
        <LoaderCircle aria-hidden="true" className="tool-spinner" />
        Finish signing in in your browser
      </span>
      <OpenPage url={url} small />
      <Button type="button" size="sm" variant="outline" onClick={onCancel}>
        Cancel
      </Button>
    </>
  );
}

/** The sign-in didn't work: why, its reference, and Retry. */
export function ToolSignInFailed({
  why,
  reference,
  onRetry,
  onReference,
}: {
  why: string;
  reference?: string;
  onRetry: () => void;
  onReference: (ref: string) => void;
}) {
  return (
    <Notice
      danger
      title="Sign-in failed"
      reference={reference ? { ref: reference, open: onReference } : undefined}
      links={<span className="muted">{why}</span>}
      action={
        <Button type="button" size="sm" variant="outline" onClick={onRetry}>
          Retry
        </Button>
      }
    />
  );
}
