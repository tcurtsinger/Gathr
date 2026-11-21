import { code, type HighlightResult } from '@streamdown/code';
import { type CSSProperties, Fragment, useEffect, useState } from 'react';
import { Markdown } from '../components/Markdown';
import './filebody.css';

type Language = Parameters<typeof code.highlight>[0]['language'];

const ending = (name: string) => (name.includes('.') ? (name.split('.').pop()?.toLowerCase() ?? '') : '');
const SEPARATORS: Record<string, string> = { csv: ',', tsv: '\t' };

/** Markdown and tables have a look apart from their text, so their tab has Source. */
export const hasLook = (name: string) => ['md', 'markdown', 'csv', 'tsv'].includes(ending(name));

/** A file's text as it looks: Markdown as a page, CSV and TSV as a table, any other as numbered lines,
 * coloured when the highlighter knows its ending. With `source`, its own text as plain numbered lines. */
export function FileBody({ name, text, source }: { name: string; text: string; source: boolean }) {
  const kind = ending(name);
  if (source) return <Lines text={text} />;
  if (kind === 'md' || kind === 'markdown')
    return (
      <div className="filebody-page">
        <Markdown text={text} className="filebody-md" />
      </div>
    );
  const separator = SEPARATORS[kind];
  const rows = separator ? readTable(text, separator) : null;
  if (rows) return <Table rows={rows} />;
  return <Lines text={text} language={code.supportsLanguage(kind as Language) ? kind : undefined} />;
}

/** The highlighter's time grows with the text's size (12 s for 2 MB of TypeScript), so past about a second's worth
 * the lines show plain. Size, not lines: a minified file is one long line. */
const COLOURED = 150_000;

/** Numbered lines that scroll sideways, coloured by the chat's code highlighter (Shiki, its themes) once it has them. */
function Lines({ text, language }: { text: string; language?: string }) {
  // A file's last newline ends its last line; it doesn't start another.
  const body = text.replace(/\r?\n$/, '');
  const count = body.split('\n').length;
  const coloured = body.length <= COLOURED ? language : undefined;
  const [colours, setColours] = useState<HighlightResult>();
  useEffect(() => {
    setColours(undefined);
    if (!coloured) return;
    let live = true;
    const themes = code.getThemes();
    const now = code.highlight({ code: body, language: coloured as Language, themes }, (r) => {
      if (live) setColours(r);
    });
    if (now) setColours(now);
    return () => {
      live = false;
    };
  }, [body, coloured]);
  return (
    <div className="filebody-lines">
      <pre className="filebody-numbers" aria-hidden="true">
        {Array.from({ length: count }, (_, i) => i + 1).join('\n')}
      </pre>
      <pre className="filebody-text">
        {colours
          ? colours.tokens.map((line, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a line's place is what it is
              <Fragment key={i}>
                {line.map((t, j) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: a token's place is what it is
                  <span key={j} style={colour(t.htmlStyle)}>
                    {t.content}
                  </span>
                ))}
                {'\n'}
              </Fragment>
            ))
          : body}
      </pre>
    </div>
  );
}

/** A token's colour in light, and Shiki's own variable for it in dark (filebody.css picks one). */
const colour = (style: Record<string, string> = {}) =>
  Object.fromEntries(Object.entries(style).map(([k, v]) => [k === 'color' ? '--l' : k, v])) as CSSProperties;

const NUMBER = /^[-+]?[\d,]*\.?\d+%?$/;

/** Past this many rows a table held the window for seconds (4 s for 49,000 rows); the rest are in Source. */
const ROWS = 2000;

/** The first row as the heading row, a column of numbers set to the right. */
function Table({ rows }: { rows: string[][] }) {
  const [head, ...body] = rows;
  const right = head.map(
    (_, c) => body.some((r) => r[c]) && body.every((r) => !r[c] || NUMBER.test(r[c].trim())),
  );
  const align = (c: number) => (right[c] ? 'right' : undefined);
  return (
    <div className="filebody-table">
      <table>
        <thead>
          <tr>
            {head.map((cell, c) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a column's place is what it is
              <th key={c} style={{ textAlign: align(c) }}>
                {cell}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {body.slice(0, ROWS).map((row, r) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: a row's place is what it is
            <tr key={r}>
              {row.map((cell, c) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: a column's place is what it is
                <td key={c} style={{ textAlign: align(c) }}>
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {body.length > ROWS && (
        <p className="filebody-more">{(body.length - ROWS).toLocaleString()} more rows</p>
      )}
    </div>
  );
}

/**
 * A CSV or TSV file's rows: a field in double quotes may hold the separator, a line break or a doubled quote; lines end
 * in \n or \r\n. Null when it doesn't read as a table: a quote left open, or rows of different lengths.
 */
export function readTable(text: string, separator: string): string[][] | null {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let quoted = false;
  const end = () => {
    row.push(cell);
    // A blank line is no row.
    if (row.length > 1 || row[0] !== '') rows.push(row);
    row = [];
    cell = '';
  };
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c !== '"') cell += c;
      else if (text[i + 1] === '"') cell += text[i++];
      else quoted = false;
    } else if (c === '"' && cell === '') quoted = true;
    else if (c === separator) {
      row.push(cell);
      cell = '';
    } else if (c === '\n' || c === '\r') {
      if (c === '\r' && text[i + 1] === '\n') i++;
      end();
    } else cell += c;
  }
  if (quoted) return null;
  if (cell || row.length) end();
  if (!rows.length || rows.some((r) => r.length !== rows[0].length)) return null;
  return rows;
}
