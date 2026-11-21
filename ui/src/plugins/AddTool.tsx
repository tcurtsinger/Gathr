import { ChevronLeft, LoaderCircle } from 'lucide-react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure } from '../api';

interface Props {
  onBack: () => void;
  /** The tool was added: its id, once the list has it. */
  onAdded: (id: string) => void;
}

/**
 * Add a tool: its link, and its name unless the tool's own will do. Gathr asks the tool what it needs; one that
 * wants a key gets the Key field; so does one that signs in through the browser, which Gathr can't do yet.
 */
export function AddTool({ onBack, onAdded }: Props) {
  const [link, setLink] = useState('');
  const [name, setName] = useState('');
  const [key, setKey] = useState('');
  const [needsKey, setNeedsKey] = useState(false);
  /** It signs in through the browser: some such tools take a key as well, so the Key field is still offered. */
  const [signsIn, setSignsIn] = useState(false);
  const [checking, setChecking] = useState(false);
  /** Why, under the field it's about. */
  const [why, setWhy] = useState<{ field: 'link' | 'key'; text: string } | null>(null);
  const add = () => {
    setChecking(true);
    setWhy(null);
    const tool = { link: link.trim(), name: name.trim() || undefined, key: needsKey ? key : undefined };
    api.addTool(tool).then(
      (answer) => {
        setChecking(false);
        if (answer.tool) onAdded(answer.tool.id);
        else {
          setNeedsKey(true);
          setSignsIn(answer.needs === 'sign_in');
        }
      },
      (f: Failure) => {
        setChecking(false);
        const text = f.why ?? f.error;
        setWhy({ field: needsKey && text.includes('key') ? 'key' : 'link', text });
      },
    );
  };
  const under = (field: 'link' | 'key') => why?.field === field && <p className="tool-why">{why.text}</p>;
  return (
    <form
      className="new-skill tool-form"
      onSubmit={(e) => {
        e.preventDefault();
        add();
      }}
    >
      <button type="button" className="plugins-back" onClick={onBack}>
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
            setNeedsKey(false);
            setSignsIn(false);
            setWhy(null);
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
          {under('key') || (
            <p className="muted">
              {signsIn
                ? 'This tool signs in through your browser, which Gathr can’t do yet. If it also takes a key, enter one.'
                : 'The key stays in Gathr. Agents never see it.'}
            </p>
          )}
        </>
      )}
      <footer className="agent-actions">
        {checking && (
          <span className="tool-checking muted">
            <LoaderCircle aria-hidden="true" className="tool-spinner" />
            Checking the link
          </span>
        )}
        <Button type="button" size="sm" variant="outline" onClick={onBack}>
          Cancel
        </Button>
        <Button type="submit" size="sm" disabled={checking || !link.trim() || (needsKey && !key.trim())}>
          {needsKey ? 'Connect' : 'Add'}
        </Button>
      </footer>
    </form>
  );
}
