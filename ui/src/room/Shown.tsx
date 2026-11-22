import { Chart } from '@/components/assistant-ui/elements/chart';
import {
  DataTable,
  type DataTableFormat,
  type DataTableRow,
} from '@/components/assistant-ui/elements/data-table';
import type { Block, Cell } from '../api';
import './shown.css';

type Table = NonNullable<Block['table']>;
type Column = Table['columns'][number];

/** How a column's values read, in the table's own words. A date is written here, so it has no hover text. */
function format(c: Column): DataTableFormat {
  if (c.as === 'number') return { kind: 'number', decimals: c.decimals, unit: c.unit };
  if (c.as === 'money') return { kind: 'currency', currency: c.currency ?? 'USD', decimals: c.decimals };
  if (c.as === 'percent') return { kind: 'percent', decimals: c.decimals, basis: 'unit' };
  if (c.as === 'change') return { kind: 'delta', decimals: c.decimals, unit: c.unit, upIsGood: c.up_is_good };
  if (c.as === 'link') return { kind: 'link' };
  if (c.as === 'yes_no') return { kind: 'boolean' };
  return { kind: 'text' };
}

/** "Nov 3, 2026" for a day, with the time in the person's clock when it has one. */
function dated(value: Cell) {
  const time = typeof value === 'string' ? Date.parse(value) : Number.NaN;
  if (Number.isNaN(time)) return value;
  const day = /^\d{4}-\d{2}-\d{2}$/.test(value as string);
  const style: Intl.DateTimeFormatOptions = day
    ? { dateStyle: 'medium', timeZone: 'UTC' }
    : { dateStyle: 'medium', timeStyle: 'short' };
  return new Intl.DateTimeFormat(undefined, style).format(time);
}

/** An empty cell is blank: the table would write "none" in it. */
function rows(table: Table): DataTableRow[] {
  return table.rows.map((row) =>
    Object.fromEntries(
      table.columns.map((c) => {
        const value = row[c.key] ?? '';
        return [c.key, c.as === 'date' ? dated(value) : value];
      }),
    ),
  );
}

/** One block as its kind draws it; a kind this window doesn't know draws nothing. */
function draw(block: Block) {
  const { table, chart } = block;
  if (block.kind === 'table' && Array.isArray(table?.columns) && Array.isArray(table.rows)) {
    // A date is written as words here, which sort by their letters: its column has no sort.
    const columns = table.columns.map((c) => ({
      key: c.key,
      label: c.label,
      format: format(c),
      sortable: c.as !== 'date',
    }));
    return (
      <DataTable
        className="[&_a]:wrap-anywhere [&_td]:wrap-break-word"
        columns={columns}
        rows={rows(table)}
        locale={navigator.language}
        caption={block.title}
      />
    );
  }
  if (block.kind === 'chart' && Array.isArray(chart?.points) && chart.points.length > 0) {
    return (
      <Chart
        label={chart.label}
        value={chart.value}
        delta={chart.change}
        points={chart.points}
        visibleCount={chart.points.length}
        variant={chart.style ?? 'area'}
        upIsGood={chart.up_is_good ?? true}
      />
    );
  }
  return null;
}

/** The tables and charts an agent showed in its turn, under its words, each with its title as a quiet line above. */
export function Shown({ blocks }: { blocks: Block[] }) {
  const drawn = blocks.map((b) => [b, draw(b)] as const).filter(([, d]) => d);
  if (!drawn.length) return null;
  return (
    <div className="shown">
      {drawn.map(([b, d], i) => (
        // biome-ignore lint/suspicious/noArrayIndexKey: a turn's blocks only grow, in order
        <div key={i} className="shown-block">
          {b.title && <div className="shown-title">{b.title}</div>}
          {d}
        </div>
      ))}
    </div>
  );
}
