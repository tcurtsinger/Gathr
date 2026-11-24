import { SquareArrowOutUpRight } from 'lucide-react';
import { type ReactNode, useContext, useEffect, useRef, useState } from 'react';
import { Chart } from '@/components/assistant-ui/elements/chart';
import {
  DataTable,
  type DataTableColumn,
  type DataTableFormat,
  type DataTableRow,
} from '@/components/assistant-ui/elements/data-table';
import { MathBlock, Sub, Sup } from '@/components/assistant-ui/elements/math-block';
import type { Block, Cell } from '../api';
import { fileName, OpenFile } from '../components/files';
import { IconButton } from '../components/IconButton';
import { Drawing } from './Drawing';
import { Flow } from './Flow';
import { PageFrame } from './PageFrame';
import { AgentProposal, Proposal } from './Proposal';
import { AgentChangeCard, ProjectChangeCard } from './ProposedChange';
import './shown.css';

type Table = NonNullable<Block['table']>;
type Column = Table['columns'][number];

/** A date with a time after its day: "2026-11-22T14:00" or "2026-11-22 14:00". */
const timed = (value: Cell) => typeof value === 'string' && value.length > 10;

/** How a column's values read, in the table's own words. A day shows as given, a time in the person's own clock. */
function format(c: Column, rows: Table['rows']): DataTableFormat {
  if (c.as === 'number') return { kind: 'number', decimals: c.decimals, unit: c.unit };
  if (c.as === 'money') return { kind: 'currency', currency: c.currency ?? 'USD', decimals: c.decimals };
  if (c.as === 'percent') return { kind: 'percent', decimals: c.decimals, basis: 'unit' };
  if (c.as === 'change') return { kind: 'delta', decimals: c.decimals, unit: c.unit, upIsGood: c.up_is_good };
  if (c.as === 'link') return { kind: 'link', hrefKey: hrefKey(c) };
  if (c.as === 'yes_no') return { kind: 'boolean' };
  if (c.as === 'date') {
    const given = rows.map((r) => r[c.key]).filter((v) => v != null);
    return { kind: 'date', style: given.length && given.every(timed) ? 'datetime' : 'date' };
  }
  return { kind: 'text' };
}

/** The hidden key that holds a link column's address. */
const hrefKey = (c: Column) => `${c.key}\u0000href`;

/** "seeds.example/tomato": a bare address as its host and path, cut in the middle when long. */
function short(address: string) {
  let url: URL;
  try {
    url = new URL(address);
  } catch {
    return address;
  }
  let path = url.pathname.replace(/\/$/, '');
  try {
    path = decodeURI(path);
  } catch {}
  const host = url.host.replace(/^www\./, '');
  const whole = host + path;
  if (whole.length <= 32) return whole;
  const cut = `${host}/…/${path.slice(path.lastIndexOf('/') + 1)}`;
  return cut.length <= 32 ? cut : `${whole.slice(0, 15)}…${whole.slice(-16)}`;
}

/** A link cell as its name and its address: "[Seed shop](https://…)" or a bare address. */
function link(value: string): [string, string] {
  const named = /^\[(.*)\]\((.*)\)$/s.exec(value);
  if (named) return [named[1], named[2]];
  return [short(value), value];
}

/** Each row as the table takes it: an empty cell is null, a link has its address under a hidden key. */
function rows(table: Table): DataTableRow[] {
  return table.rows.map((row) =>
    Object.fromEntries(
      table.columns.flatMap((c) => {
        const value = row[c.key] ?? null;
        if (c.as === 'date' && typeof value === 'string') return [[c.key, value.replace(' ', 'T')]];
        if (c.as !== 'link' || typeof value !== 'string') return [[c.key, value]];
        const [name, address] = link(value);
        const http = /^https?:\/\//.test(address);
        return [
          [c.key, name],
          [hrefKey(c), http ? address : null],
        ];
      }),
    ),
  );
}

function table(block: Block, t: Table) {
  const columns: DataTableColumn[] = t.columns.map((c) => ({
    key: c.key,
    label: c.label,
    format: format(c, t.rows),
  }));
  // A link and a date stay on one line; a table too wide for the card scrolls sideways.
  return (
    <DataTable
      className="overflow-x-auto [&_a]:whitespace-nowrap [&_td]:wrap-break-word [&_td>span]:whitespace-nowrap"
      columns={columns}
      rows={rows(t)}
      locale={navigator.language}
      caption={block.title}
    />
  );
}

/** `x^2`, `x^{n+1}`, `a_1`, `a_{ij}` as raised and lowered text; anything else as typed. */
function expression(text: string) {
  const parts: ReactNode[] = [];
  let plain = '';
  let i = 0;
  while (i < text.length) {
    const mark = text[i];
    const next = text[i + 1] ?? '';
    let raised = '';
    let end = i;
    if (mark === '^' || mark === '_') {
      if (next === '{') {
        const close = text.indexOf('}', i + 2);
        if (close > i + 2) [raised, end] = [text.slice(i + 2, close), close + 1];
      } else if (/\d/.test(next)) {
        raised = /^\d+/.exec(text.slice(i + 1))?.[0] ?? '';
        end = i + 1 + raised.length;
      } else if (/\p{L}/u.test(next)) {
        [raised, end] = [next, i + 2];
      }
    }
    if (!raised) {
      plain += mark;
      i += 1;
      continue;
    }
    const Mark = mark === '^' ? Sup : Sub;
    parts.push(plain, <Mark key={i}>{raised}</Mark>);
    plain = '';
    i = end;
  }
  parts.push(plain);
  return parts;
}

/**
 * An agent's page in its sealed frame (`PageFrame`), drawn only while it is on or near the screen, so a long chat with
 * many pages stays light; Open shows it as a tab in the chat's panel.
 */
function Page({ chat, page, title }: { chat: string; page: NonNullable<Block['page']>; title?: string }) {
  const open = useContext(OpenFile);
  const box = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(false);
  useEffect(() => {
    const at = box.current;
    if (!at) return;
    const seen = new IntersectionObserver(([e]) => setNear(e.isIntersecting), { rootMargin: '50% 0px' });
    seen.observe(at);
    return () => seen.disconnect();
  }, []);
  return (
    <div ref={box} className="shown-page" style={{ height: page.height ?? 360 }}>
      {near && <PageFrame chat={chat} path={page.file} name={title || fileName(page.file)} />}
      {open && (
        <IconButton aria-label="Open" className="shown-page-open" onClick={() => open(page.file)}>
          <SquareArrowOutUpRight aria-hidden="true" />
        </IconButton>
      )}
    </div>
  );
}

/** One block as its kind draws it; a kind this window doesn't know draws nothing. */
function draw(chat: string, block: Block) {
  const { table: t, chart, flow: f, math, diagram, page } = block;
  if (block.kind === 'table' && Array.isArray(t?.columns) && Array.isArray(t.rows)) return table(block, t);
  if (block.kind === 'chart' && Array.isArray(chart?.points) && chart.points.length > 0) {
    return (
      <Chart
        label={chart.label}
        value={chart.value}
        delta={chart.change}
        against={typeof chart.vs === 'string' ? chart.vs : undefined}
        title={block.title}
        names={Array.isArray(chart.labels) ? chart.labels.map(String) : undefined}
        points={chart.points}
        visibleCount={chart.points.length}
        variant={chart.style ?? 'area'}
        upIsGood={chart.up_is_good ?? true}
      />
    );
  }
  if (block.kind === 'flow' && Array.isArray(f?.nodes) && f.nodes.length > 0) return <Flow nodes={f.nodes} />;
  if (block.kind === 'math' && Array.isArray(math?.steps)) {
    const steps = math.steps.map((s) => ({ expression: expression(String(s.expression)), note: s.note }));
    return <MathBlock label={math.label} steps={steps} visibleSteps={steps.length} />;
  }
  if (
    block.kind === 'diagram' &&
    (typeof diagram?.mermaid === 'string' || typeof diagram?.file === 'string')
  ) {
    return <Drawing chat={chat} diagram={diagram} title={block.title} />;
  }
  if (block.kind === 'page' && typeof page?.file === 'string')
    return <Page chat={chat} page={page} title={block.title} />;
  if (block.kind === 'project' && block.message && Array.isArray(block.project?.team))
    return <Proposal message={block.message} project={block.project} />;
  if (block.kind === 'agent' && block.message && typeof block.agent?.name === 'string')
    return <AgentProposal message={block.message} agent={block.agent} />;
  if (block.kind === 'agent_change' && block.message && block.agent_change?.was)
    return <AgentChangeCard message={block.message} change={block.agent_change} />;
  if (block.kind === 'project_change' && block.message && Array.isArray(block.project_change?.to?.team))
    return <ProjectChangeCard message={block.message} change={block.project_change} />;
  return null;
}

/**
 * What an agent showed in its turn, under its words, each with its title as a quiet line above (a chart's and a
 * diagram's is in their own head).
 * Charts that follow one another sit side by side in one wrapping row.
 */
export function Shown({ chat, blocks }: { chat: string; blocks: Block[] }) {
  const runs: { chart: boolean; items: [number, Block, ReactNode][] }[] = [];
  blocks.forEach((b, i) => {
    const drawn = draw(chat, b);
    if (!drawn) return;
    const chart = b.kind === 'chart';
    const last = runs.at(-1);
    if (chart && last?.chart) last.items.push([i, b, drawn]);
    else runs.push({ chart, items: [[i, b, drawn]] });
  });
  if (!runs.length) return null;
  const item = ([i, b, d]: [number, Block, ReactNode]) => (
    <div key={i} className="shown-block">
      {b.title && b.kind !== 'diagram' && b.kind !== 'chart' && <div className="shown-title">{b.title}</div>}
      {d}
    </div>
  );
  return (
    <div className="shown">
      {runs.map((run) =>
        run.chart ? (
          <div key={run.items[0][0]} className="shown-row">
            {run.items.map(item)}
          </div>
        ) : (
          item(run.items[0])
        ),
      )}
    </div>
  );
}
