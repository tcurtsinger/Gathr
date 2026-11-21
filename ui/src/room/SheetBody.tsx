import { useEffect, useState } from 'react';
import { api, type Failure, type Sheet, type Unshown } from '../api';
import { Table } from './FileBody';
import './documents.css';

interface Props {
  chat: string;
  path: string;
  onUnshown: (why: Unshown) => void;
  onFailure: (failure: Failure) => void;
}

/** A workbook's first sheet as a table, with a small tab per sheet when it has more than one. */
export function SheetBody({ chat, path, onUnshown, onFailure }: Props) {
  const [sheets, setSheets] = useState<Sheet[]>();
  const [at, setAt] = useState(0);
  useEffect(() => {
    let live = true;
    api.sheets(chat, path).then(
      (got) => {
        if (!live) return;
        if ('unshown' in got) onUnshown(got.unshown);
        else if (!got.sheets.length) onUnshown('cant');
        else setSheets(got.sheets);
      },
      (f: Failure) => live && onFailure(f),
    );
    return () => {
      live = false;
    };
  }, [chat, path, onUnshown, onFailure]);
  const sheet = sheets?.[at];
  return (
    <>
      {sheets && sheets.length > 1 && (
        <div className="sheet-tabs" role="tablist" aria-label="Sheets">
          {sheets.map((s, i) => (
            <div key={s.name} className="workspace-tab" data-selected={i === at}>
              <button type="button" role="tab" aria-selected={i === at} onClick={() => setAt(i)}>
                <span>{s.name}</span>
              </button>
            </div>
          ))}
        </div>
      )}
      {sheet && sheet.rows.length > 0 && <Table key={sheet.name} rows={sheet.rows} total={sheet.total} />}
    </>
  );
}
