import { useEffect, useState } from 'react';
import { api, type BackgroundChoice, type Failure } from '../api';
import { useCard } from '../components/Corner';
import { Dropdown } from '../components/Dropdown';
import { Page, Row } from './Row';
import { useSettings } from './saved';

/** Gathr's own pick, as the setting saves it: none. */
const AUTO = 'auto';
const key = (c: { account: string; model: string }) => `${c.account}\t${c.model}`;
const named = (c: BackgroundChoice) => `${c.name} · ${c.account_name}`;

/** Settings > Background: the model for work nobody waits on, which names new chats. */
export function Background() {
  const { saved, save } = useSettings();
  const [listed, setListed] = useState<{ auto: BackgroundChoice | null; choices: BackgroundChoice[] } | null>(
    null,
  );
  const [failure, setFailure] = useState<Failure | null>(null);
  useCard(failure);
  useEffect(() => {
    api.background().then(setListed, setFailure);
  }, []);

  const picked = saved?.background_model ?? null;
  const none = listed && !listed.auto && !picked;
  return (
    <Page title="Background">
      {saved && listed && (
        <div className="settings-rows">
          <Row
            name={<label htmlFor="background-model">Model</label>}
            line={
              none
                ? 'No account has a small model.'
                : 'A small model for work nobody waits on. It uses that account’s usage.'
            }
            lineId="background-line"
          >
            <Dropdown
              id="background-model"
              aria-describedby="background-line"
              value={picked ? key(picked) : AUTO}
              onChange={(v) => {
                const choice = listed.choices.find((c) => key(c) === v);
                save({ background_model: choice ? { account: choice.account, model: choice.model } : null });
              }}
              choices={[
                { value: AUTO, label: listed.auto ? named(listed.auto) : 'None' },
                ...listed.choices.map((c) => ({ value: key(c), label: named(c) })),
                // A pick whose account is signed out now, or no longer lists it: as it was saved.
                ...(picked && !listed.choices.some((c) => key(c) === key(picked))
                  ? [{ value: key(picked), label: picked.model }]
                  : []),
              ]}
            />
          </Row>
        </div>
      )}
    </Page>
  );
}
