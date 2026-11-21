import { useCallback, useEffect, useId, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, type Engine, type Failure } from '../api';
import { FailureNotice } from '../components/Notice';
import { Accounts, engines } from './Accounts';
import { Group, Page, Row } from './Row';
import './engines.css';

type Open = (ref: string) => void;

/** Settings > Engines (settings-entry/v5): the accounts, then each engine's program once. */
export function Engines({ onReference }: { onReference: Open }) {
  return (
    <Page title="Engines" line="The accounts your agents sign in with, and each engine’s version.">
      <Accounts onReference={onReference} />
      <Group title="Versions" />
      <div className="settings-rows settings-top">
        {engines.map(([id, name]) => (
          <Program key={id} id={id} name={name} onReference={onReference} />
        ))}
      </div>
    </Page>
  );
}

/** Gathr's copy of one engine: its version and the release check (0025). */
function Program({ id, name, onReference }: { id: string; name: string; onReference: Open }) {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const title = useId();

  const load = useCallback(() => api.engine(id).then(setEngine, setFailure), [id]);
  useEffect(() => {
    load();
  }, [load]);
  // A check sends no live event: look again after each answer, until one says it ended.
  const checking = engine?.copy.state === 'checking';
  useEffect(() => {
    if (engine?.copy.state !== 'checking') return;
    const again = setTimeout(load, 1000);
    return () => clearTimeout(again);
  }, [engine, load]);

  const check = () => {
    setFailure(null);
    api.checkEngine(id).then(load, setFailure);
  };

  const copy = engine?.copy;
  const shown = failure ?? (copy?.failed && { error: 'Update failed', ...copy.failed });
  const notice = shown && <FailureNotice failure={shown} onReference={onReference} />;
  if (!engine) return notice;
  const status = checking
    ? 'Checking'
    : copy?.state === 'off'
      ? 'Updates disabled'
      : copy?.checked && copy.version && !copy.failed
        ? 'Up to date'
        : '';
  const version = copy?.version ?? (engine.found ? '' : 'Not installed');
  return (
    <Row
      name={<span id={title}>{name}</span>}
      line={
        <>
          {version}
          {version && status && ' · '}
          <span role="status">{status}</span>
        </>
      }
      after={
        <>
          {notice}
          {engine.why_not_found && <p className="field-error">{engine.why_not_found}</p>}
        </>
      }
    >
      {copy?.state !== 'off' && (
        <Button type="button" variant="outline" aria-describedby={title} disabled={checking} onClick={check}>
          Check
        </Button>
      )}
    </Row>
  );
}
