import type { PcRuns } from '../api';

/** A variable's value is never shown, nor sent to the window: dots stand in for it. */
const DOTS = '••••••••••••';

/**
 * What a tool on this PC runs: the program's full path, or that it isn't found; each argument on its own line; its
 * folder; each variable by name; and where it comes from, when that's known.
 */
export function Runs({ runs, from }: { runs: PcRuns; from?: string | null }) {
  return (
    <dl className="tool-runs">
      <dt>Program</dt>
      <dd>
        {runs.program ? (
          <code>{runs.program}</code>
        ) : (
          <code data-tone="attention">{runs.typed} · not found</code>
        )}
      </dd>
      {runs.args.length > 0 && (
        <>
          <dt>Arguments</dt>
          <dd>
            <code>{runs.args.join('\n')}</code>
          </dd>
        </>
      )}
      {runs.folder && (
        <>
          <dt>Folder</dt>
          <dd>
            <code>{runs.folder}</code>
          </dd>
        </>
      )}
      {runs.variables.length > 0 && (
        <>
          <dt>Variables</dt>
          <dd>
            <code>{runs.variables.map((name) => `${name}  ${DOTS}`).join('\n')}</code>
          </dd>
        </>
      )}
      {from && (
        <>
          <dt>From</dt>
          <dd>{from}</dd>
        </>
      )}
    </dl>
  );
}
