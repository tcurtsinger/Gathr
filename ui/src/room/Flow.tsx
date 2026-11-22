import { useLayoutEffect, useRef, useState } from 'react';
import { mono, paper } from '@/components/assistant-ui/elements/surfaces';
import type { Block } from '../api';

type Node = NonNullable<Block['flow']>['nodes'][number];

/** Each node's place: its column one past the furthest it follows, rows in the order given. */
function place(nodes: Node[]) {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const columns = new Map<string, number>();
  const column = (id: string, seen: Set<string>): number => {
    const known = columns.get(id);
    if (known !== undefined) return known;
    const node = byId.get(id);
    if (!node || seen.has(id)) return 0;
    seen.add(id);
    const after = (node.after ?? []).filter((a) => byId.has(a));
    const at = after.length ? Math.max(...after.map((a) => column(a, seen))) + 1 : 0;
    columns.set(id, at);
    return at;
  };
  const filled = new Map<number, number>();
  return nodes.map((n) => {
    const col = column(n.id, new Set());
    const row = filled.get(col) ?? 0;
    filled.set(col, row + 1);
    return { col, row };
  });
}

// The registry's flow graph's look, as published.
const look = {
  done: 'border-foreground/10 bg-foreground/[0.04] text-muted-foreground',
  active: 'text-foreground/90 border-blue-500/30 bg-blue-500/10 dark:border-blue-400/30',
  pending: 'border-border text-muted-foreground border-dashed',
};

/**
 * Work as boxes 120px wide that grow down to hold their words, each centred on its row. A grid places them; the lines
 * are drawn from where the boxes landed, read again whenever a box or the grid changes size (as when the font arrives).
 */
export function Flow({ nodes }: { nodes: Node[] }) {
  const grid = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState<{ x0: number; x1: number; y: number }[]>([]);
  useLayoutEffect(() => {
    const at = grid.current;
    if (!at) return;
    // The lines come first, then a box for each node.
    const boxes = nodes.map((_, i) => at.children[i + 1] as HTMLElement);
    const read = () =>
      setEdges(
        boxes.map((b) => ({
          x0: b.offsetLeft,
          x1: b.offsetLeft + b.offsetWidth,
          y: b.offsetTop + b.offsetHeight / 2,
        })),
      );
    const sized = new ResizeObserver(read);
    for (const el of [at, ...boxes]) sized.observe(el);
    return () => sized.disconnect();
  }, [nodes]);
  const places = place(nodes);
  const index = new Map(nodes.map((n, i) => [n.id, i]));
  const lines = nodes.flatMap((n, i) =>
    (n.after ?? []).flatMap((from) => {
      const a = edges[index.get(from) ?? -1];
      const b = edges[i];
      if (!a || !b) return [];
      const mid = (a.x1 + b.x0) / 2;
      return [
        { key: `${from}-${n.id}`, d: `M ${a.x1} ${a.y} C ${mid} ${a.y}, ${mid} ${b.y}, ${b.x0} ${b.y}` },
      ];
    }),
  );
  const cols = Math.max(...places.map((p) => p.col)) + 1;
  // As wide as its columns, up to the card's width, past which it scrolls.
  return (
    <div className={`${paper} w-fit max-w-full overflow-x-auto rounded-2xl p-4`}>
      <div
        ref={grid}
        className="relative grid w-max items-center gap-x-[18px] gap-y-[28px]"
        style={{ gridTemplateColumns: `repeat(${cols}, 120px)` }}
      >
        <svg aria-hidden className="absolute inset-0 size-full overflow-visible">
          {lines.map(({ key, d }) => (
            <path key={key} d={d} fill="none" strokeWidth="1.5" className="stroke-foreground/20" />
          ))}
        </svg>
        {nodes.map((n, i) => (
          <div
            key={n.id}
            className={`relative flex min-h-[30px] items-center justify-center rounded-xl border px-2 py-1.5 text-center text-[11.5px] leading-tight wrap-anywhere ${look[n.state ?? 'pending']}`}
            style={{ gridColumn: places[i].col + 1, gridRow: places[i].row + 1 }}
          >
            <span className={mono}>{n.label}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
