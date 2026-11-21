import { ChevronLeft, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';
import { ToolSignInFailed, ToolSignInWaiting, useToolSignIn } from './ToolSignIn';
import { host } from './tool';

interface Props {
  onBack: () => void;
  /** The tool was added: its id, once the list has it. */
  onAdded: (id: string) => void;
  onReference: (ref: string) => void;
}

/**
 * Add a tool: its link, and its name unless the tool's own will do. Gathr asks the tool what it needs: one that
 * wants a key gets the Key field; one that signs in through the browser gets Sign in, and Use a key for one that
 * takes a key as well.
 */
export function AddTool({ onBack, onAdded, onReference }: Props) {
  const [link, setLink] = useState('');
  const [name, setName] = useState('');
  const [key, setKey] = useState('');
  const [needs, setNeeds] = useState<'key' | 'sign_in' | null>(null);
  const [checking, setChecking] = useState(false);
  /** Why, under the field it's about. */
  const [why, setWhy] = useState<{ field: 'link' | 'key'; text: string } | null>(null);
  const signIn = useToolSignIn(onAdded);
  const needsKey = needs === 'key';
  const add = () => {
    setChecking(true);
    setWhy(null);
    const tool = { link: link.trim(), name: name.trim() || undefined, key: needsKey ? key : undefined };
    api.addTool(tool).then(
      (answer) => {
        setChecking(false);
        if (answer.tool) onAdded(answer.tool.id);
        else setNeeds(answer.needs ?? 'key');
      },
      (f: Failure) => {
        setChecking(false);
        const text = f.why ?? f.error;
        setWhy({ field: needsKey && text.includes('key') ? 'key' : 'link', text });
      },
    );
  };
  const under = (field: 'link' | 'key') => why?.field === field && <p className="tool-why">{why.text}</p>;
  const footer = () => {
    if (signIn.waiting) return <ToolSignInWaiting url={signIn.waiting.url} onCancel={signIn.cancel} />;
    const cancel = (
      <Button type="button" size="sm" variant="outline" onClick={onBack}>
        Cancel
      </Button>
    );
    if (signIn.failed) return cancel;
    if (needs === 'sign_in')
      return (
        <>
          <Button
            type="button"
            size="sm"
            variant="ghost"
            className="tool-use-key"
            onClick={() => setNeeds('key')}
          >
            Use a key
          </Button>
          {cancel}
          <Button
            type="button"
            size="sm"
            onClick={() => signIn.start({ link: link.trim(), name: name.trim() || undefined })}
          >
            Sign in
          </Button>
        </>
      );
    return (
      <>
        {checking && (
          <span className="tool-checking muted">
            <LoaderCircle aria-hidden="true" className="tool-spinner" />
            Checking the link
          </span>
        )}
        {cancel}
        <Button type="submit" size="sm" disabled={checking || !link.trim() || (needsKey && !key.trim())}>
          {needsKey ? 'Connect' : 'Add'}
        </Button>
      </>
    );
  };
  return (
    <form
      className="new-skill tool-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (needs !== 'sign_in') add();
      }}
    >
      <button type="button" className="screen-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Plugins
      </button>
      <h1>Add a tool</h1>
      <p className="muted">Paste the tool’s link. Gathr asks the tool what it needs.</p>
      <label className="agent-field">
        Link
        <input
          className="tool-link"
          value={link}
          onChange={(e) => {
            setLink(e.target.value);
            setNeeds(null);
            setWhy(null);
            signIn.cancel();
          }}
          aria-invalid={why?.field === 'link' || undefined}
          spellCheck={false}
          placeholder="https://"
          // biome-ignore lint/a11y/noAutofocus: the page opens from Add, to paste the link.
          autoFocus
        />
      </label>
      {under('link')}
      <label className="agent-field">
        Name
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Filled in from the tool" />
      </label>
      {needsKey && (
        <>
          <label className="agent-field">
            Key
            <input
              type="password"
              value={key}
              onChange={(e) => setKey(e.target.value)}
              aria-invalid={why?.field === 'key' || undefined}
              autoComplete="off"
              // biome-ignore lint/a11y/noAutofocus: the field appears when the tool asks for a key.
              autoFocus
            />
          </label>
          {under('key') || <p className="muted">The key stays in Gathr. Agents never see it.</p>}
        </>
      )}
      {needs === 'sign_in' && !signIn.waiting && !signIn.failed && (
        <p className="muted">
          {name.trim() || host(link.trim())} signs in with your account, in your browser.
        </p>
      )}
      {signIn.failed && (
        <ToolSignInFailed
          why={signIn.failed.why}
          reference={signIn.failed.ref}
          onRetry={signIn.retry}
          onReference={onReference}
        />
      )}
      <footer className="agent-actions">{footer()}</footer>
    </form>
  );
}
