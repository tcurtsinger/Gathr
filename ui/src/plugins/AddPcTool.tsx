import { ChevronLeft, LoaderCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Failure, type PcRead } from '../api';
import { Runs } from './Runs';

interface Props {
  onBack: () => void;
  /** It was saved: its id, once the list has it. */
  onDone: (id: string) => void;
  /** Change: the tool, and what it runs as a JSON block, each variable with an empty value. */
  changing?: { id: string; name: string; text: string };
}

/** How long after typing stops the text is read back. */
const PAUSE_MS = 400;

/**
 * Add a tool on this PC, or change one: the command or JSON from its instructions, read back as what Gathr will run
 * before anything runs. Run it (Save) starts it; it's saved even when it doesn't start.
 */
export function AddPcTool({ onBack, onDone, changing }: Props) {
  const [text, setText] = useState(changing?.text ?? '');
  const [name, setName] = useState(changing?.name ?? '');
  /** A name the person typed, or the tool's own on Change, isn't replaced by the one read. */
  const named = useRef(!!changing);
  const [read, setRead] = useState<PcRead | null>(null);
  const [why, setWhy] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const asked = useRef(0);

  useEffect(() => {
    const ask = ++asked.current;
    if (!text.trim()) {
      setRead(null);
      setWhy(null);
      return;
    }
    const timer = setTimeout(() => {
      api.readTool(text).then(
        (answer) => {
          if (ask !== asked.current) return;
          setRead(answer);
          setWhy(null);
          if (!named.current) setName(answer.name);
        },
        (f: Failure) => {
          if (ask !== asked.current) return;
          setRead(null);
          setWhy(f.why ?? f.error);
        },
      );
    }, PAUSE_MS);
    return () => clearTimeout(timer);
  }, [text]);

  const run = () => {
    setStarting(true);
    setWhy(null);
    const given = { text, name: name.trim() || undefined };
    const saved = changing
      ? api.changePcTool(changing.id, given).then(() => changing.id)
      : api.addPcTool(given).then((answer) => answer.tool.id);
    saved.then(onDone, (f: Failure) => {
      setStarting(false);
      setWhy(f.why ?? f.error);
    });
  };
  const found = !!read?.program;
  return (
    <form
      className="new-skill tool-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (found && !starting) run();
      }}
    >
      <button type="button" className="screen-back" onClick={onBack}>
        <ChevronLeft aria-hidden="true" />
        Plugins
      </button>
      <h1>{changing ? `Change ${changing.name}` : 'Add a tool on this PC'}</h1>
      <p className="muted">Paste the command or the JSON from the tool’s instructions.</p>
      <label className="agent-field">
        Command
        <textarea
          className="tool-command"
          value={text}
          rows={changing ? 10 : 4}
          onChange={(e) => setText(e.target.value)}
          aria-invalid={!!why || undefined}
          spellCheck={false}
          // biome-ignore lint/a11y/noAutofocus: the page opens from Add, to paste the command.
          autoFocus
        />
      </label>
      {why && <p className="tool-why">{why}</p>}
      {changing && !why && <p className="muted">A variable left empty keeps its value.</p>}
      <label className="agent-field">
        Name
        <input
          value={name}
          onChange={(e) => {
            named.current = true;
            setName(e.target.value);
          }}
          placeholder="Filled in from what you paste"
        />
      </label>
      {read && (
        <section className="tool-will-run">
          <h2>What Gathr will run</h2>
          <Runs runs={read} from={read.from} />
          {read.missing ? (
            <p data-tone="attention">{read.missing}</p>
          ) : (
            <p className="muted">
              It runs on this PC as you. It can read and change your files and reach the web, whatever its
              actions say.
            </p>
          )}
        </section>
      )}
      <footer className="agent-actions">
        {starting && (
          <span className="tool-checking muted">
            <LoaderCircle aria-hidden="true" className="tool-spinner" />
            Starting it
          </span>
        )}
        <Button type="button" size="sm" variant="outline" onClick={onBack}>
          Cancel
        </Button>
        <Button type="submit" size="sm" disabled={!found || starting}>
          {changing ? 'Save' : 'Run it'}
        </Button>
      </footer>
    </form>
  );
}
