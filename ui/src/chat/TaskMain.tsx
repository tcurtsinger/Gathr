import { useState } from 'react';
import { briefRows } from '../tasks/model';
import { MainWord } from './MainWord';
import type { Custom } from './messages';
import { AgentFace, Bubble, NameLine, RoleWord, Said, When, YouSaid } from './Said';
import { useActions } from './Thread';

/**
 * A task the lead hands out, as a main message (0075): drawn from the task's record, as no message carries the
 * hand-out. Its giver speaks to its owner: the blue @Name, the task's title (it opens the task), and the first line of
 * why it was given; More opens the rest of why and the brief, in the order the task's view shows them.
 */
export function TaskMain({ own }: { own: Custom }) {
  const actions = useActions();
  const [more, setMore] = useState(false);
  const brief = own.brief as NonNullable<Custom['brief']>;
  const [first, ...rest] = (brief.why ?? '').split('\n');
  const rows = briefRows(brief, []).filter(([, words]) => !!words);
  const longer = rest.some((l) => l.trim()) || rows.length > 0;
  const { by, agent = 'You' } = own;
  return (
    <Said
      root
      agent={by}
      you={!by}
      face={
        by ? (
          <AgentFace
            agent={by}
            name={agent}
            size={36}
            onOpen={own.role ? () => actions.openAgent(by) : undefined}
          />
        ) : (
          <YouSaid />
        )
      }
      line={
        <NameLine
          name={agent}
          pieces={[own.role === 'supervisor' && <RoleWord key="role" />, <When key="t" time={own.time} />]}
          state={<MainWord state={own.state} />}
        />
      }
    >
      <Bubble>
        <div className="task-main" data-more={more || undefined}>
          <p className="task-main-title">
            {brief.owner && <span className="mention">@{brief.owner}</span>}
            <button type="button" className="task-main-what" onClick={() => actions.openTask(brief.number)}>
              {brief.what}
            </button>
          </p>
          {(first || longer) && (
            <p className="task-main-why">
              <span className="task-main-first">{first}</span>
              {longer && (
                <button
                  type="button"
                  className="task-main-more"
                  aria-expanded={more}
                  onClick={() => setMore(!more)}
                >
                  {more ? 'Less' : 'More'}
                </button>
              )}
            </p>
          )}
          {more && (
            <div className="task-main-rest">
              {rest.some((l) => l.trim()) && <p className="task-main-why-rest">{rest.join('\n').trim()}</p>}
              {rows.length > 0 && (
                <dl>
                  {rows.map(([label, words]) => (
                    <div key={label}>
                      <dt>{label}</dt>
                      <dd>{words}</dd>
                    </div>
                  ))}
                </dl>
              )}
            </div>
          )}
        </div>
      </Bubble>
    </Said>
  );
}
