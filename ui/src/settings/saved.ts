import { useEffect, useState } from 'react';
import { api, type Failure, type Settings } from '../api';
import { useNewestWins } from '../newest';

/**
 * The saved settings, read as a page opens. `save` shows a change at once and answers whether it was taken; a
 * refused one says why and shows what's saved.
 */
export function useSettings() {
  const [saved, setSaved] = useState<Settings | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  const newest = useNewestWins();

  useEffect(() => {
    api.settings().then(setSaved, setFailure);
  }, []);

  const save = (change: Partial<Settings>) => {
    setSaved((s) => s && { ...s, ...change });
    setFailure(null);
    return newest(api.saveSettings(change)).then(
      (s) => {
        setSaved(s);
        return true;
      },
      (f: Failure) => {
        setFailure(f);
        newest(api.settings()).then(setSaved, setFailure);
        return false;
      },
    );
  };
  return { saved, failure, save };
}
