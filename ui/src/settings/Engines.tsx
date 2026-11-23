import { LoaderCircle } from 'lucide-react';
import { useCallback, useEffect, useId, useMemo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { api, downloading, type Engine, Failure } from '../api';
import { useCard } from '../components/Corner';
import { EngineLogo } from '../components/EngineLogo';
import { Accounts, engines } from './Accounts';
import { Group, Page, Row } from './Row';
import './engines.css';

/** Settings > Engines (settings-entry/v5): the accounts, then each engine's program once. */
export function Engines() {
  return (
    <Page title="Engines" line="Accounts and engine versions.">
      <Accounts />
      <Group title="Versions" />
      <div className="settings-rows settings-top">
        {engines.map(([id, name]) => (
          <Program key={id} id={id} name={name} />
        ))}
      </div>
    </Page>
  );
}

/** Gathr's copy of one engine: its version and the release check (0025). */
function Program({ id, name }: { id: string; name: string }) {
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
  const { why, ref } = copy?.failed ?? {};
  const last = useMemo(() => (ref ? new Failure('Update failed', why ?? null, ref) : null), [why, ref]);
  useCard(failure ?? last);
  if (!engine) return null;
  const status = checking
    ? 'Checking'
    : copy?.state === 'off'
      ? 'Updates disabled'
      : copy?.checked && copy.version && !copy.failed
        ? 'Up to date'
        : '';
  // Its first copy downloading is said in the line, muted, never as a failure.
  const fetching = downloading(engine.why_not_found);
  const version = copy?.version ?? (engine.found || fetching ? '' : 'Not installed');
  return (
    <Row
      name={<span id={title}>{name}</span>}
      lead={<EngineLogo engine={id} className="engine-logo" />}
      line={
        fetching ? (
          <span className="engine-fetching" role="status">
            <LoaderCircle aria-hidden="true" />
            Downloading
          </span>
        ) : (
          <>
            {version}
            {version && status && ' · '}
            <span role="status">{status}</span>
          </>
        )
      }
      after={!fetching && engine.why_not_found && <p className="field-error">{engine.why_not_found}</p>}
    >
      {copy?.state !== 'off' && (
        <Button type="button" variant="outline" aria-describedby={title} disabled={checking} onClick={check}>
          Check
        </Button>
      )}
    </Row>
  );
}
