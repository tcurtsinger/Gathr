import { useState } from 'react';
import { OptionList } from '@/components/assistant-ui/elements/option-list';
import { api, type Failure } from '../api';
import { FailureNotice } from '../components/Notice';

type Size = 'repair' | 'new';

const SIZES = [
  { id: 'repair', label: 'Just fix it.' },
  { id: 'new', label: 'Plan it with me first.' },
];

/**
 * An ask's two sizes, the current one pressed (0061 G5.4). Choosing the other asks the service; the
 * line redraws from the live update, so it shows what the service holds.
 */
export function SizeChoice({
  ask,
  size,
  onReference,
}: {
  ask: string;
  size: Size;
  onReference?: (ref: string) => void;
}) {
  const [failure, setFailure] = useState<Failure>();
  // A click on the pressed option reports it again.
  const choose = (next: Size) => {
    if (next === size) return;
    setFailure(undefined);
    api.resizeAsk(ask, next).catch(setFailure);
  };
  return (
    <>
      <OptionList
        presentation="inline"
        aria-label="Size"
        options={SIZES}
        value={[size]}
        onValueChange={([next]) => choose(next as Size)}
      />
      {failure && <FailureNotice failure={failure} onReference={onReference} />}
    </>
  );
}
