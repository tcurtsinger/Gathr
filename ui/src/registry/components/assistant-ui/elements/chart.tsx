"use client";

import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";
import { paper } from "./surfaces";
import { clamp, take } from "../utils/range";

export type ChartVariant = "area" | "line" | "bars";

const W = 300;
const H = 88;
const PAD = 6;

const scale = (points: readonly number[]) => {
  const max = Math.max(...points, 1);
  const min = Math.min(...points, 0);
  const span = max - min || 1;
  return (value: number) => H - PAD - ((value - min) / span) * (H - PAD * 2);
};

export function Chart({
  label,
  value,
  delta,
  points,
  visibleCount,
  variant = "area",
  trend,
  upIsGood = true,
  title,
  names,
  against,
  className,
  ...props
}: Omit<
  ComponentProps<"div">,
  | "children"
  | "label"
  | "value"
  | "delta"
  | "points"
  | "visibleCount"
  | "variant"
  | "trend"
  | "upIsGood"
  | "title"
> & {
  label: string;
  value: string;
  delta?: string | undefined;
  points: readonly number[];
  visibleCount: number;
  variant?: ChartVariant | undefined;
  /** The direction `delta` moved in. Without it, a leading minus sign reads as down. */
  trend?: "up" | "down" | "flat" | undefined;
  /** Whether a rise is good news, as for revenue, or bad news, as for latency. */
  upIsGood?: boolean | undefined;
  /** Gathr: the chart's name, first in its head, with `label` quiet after it. */
  title?: string | undefined;
  /** Gathr: a name for each point ("Jan", "Feb"…): the first and last under the chart, and the high point's. */
  names?: readonly string[] | undefined;
  /** Gathr: what `delta` is measured against, said after it as "vs Oct". */
  against?: string | undefined;
}) {
  const shown = take(points, clamp(visibleCount, 1, points.length));
  const y = scale(points);
  const step = points.length > 1 ? (W - PAD * 2) / (points.length - 1) : 0;
  const x = (i: number) => PAD + i * step;

  const coords = shown.map((p, i) => ({ x: x(i), y: y(p) }));
  const line = coords.map((c) => `${c.x},${c.y}`).join(" ");
  const last = coords.at(-1);
  const area = last
    ? `M ${PAD},${H - PAD} ${coords.map((c) => `L ${c.x},${c.y}`).join(" ")} L ${last.x},${H - PAD} Z`
    : "";
  const lastIndex = shown.length - 1;
  // Gathr: the points' names, when there is one for each; the high point named when it isn't the last.
  const named = names?.length === points.length ? names.slice(0, shown.length) : undefined;
  const high = Math.max(...shown);
  const highAt = shown.indexOf(high);
  const peak =
    named && highAt !== lastIndex && named[highAt]
      ? { at: highAt, value: high, name: named[highAt] }
      : undefined;
  const direction =
    trend ??
    (delta === undefined
      ? undefined
      : /\d/.test(delta) && !/[1-9]/.test(delta)
        ? "flat"
        : /^\s*[-−–]/.test(delta)
          ? "down"
          : "up");
  const good =
    direction === "up" || direction === "down"
      ? (direction === "up") === upIsGood
      : undefined;

  return (
    <div
      data-slot="chart"
      className={cn(
        paper,
        "flex w-full max-w-sm flex-col gap-3 rounded-2xl p-4",
        className,
      )}

      {...props}
    >
      <div
        className={cn(
          "flex text-[12.5px]",
          title ? "flex-col gap-0.5" : "items-baseline justify-between gap-2",
        )}
      >
        {title && <span className="text-[13px] font-medium text-balance">{title}</span>}
        <span className="text-muted-foreground">{label}</span>
      </div>

      <div className="flex flex-wrap items-baseline gap-x-2 text-[12.5px] tabular-nums">
        <span className="text-2xl font-medium tracking-tight">{value}</span>
        {delta !== undefined && (
          <span
            data-trend={direction}
            className={cn(
              "font-medium",
              good === undefined
                ? "text-muted-foreground"
                : good
                  ? "text-emerald-700 dark:text-emerald-400"
                  : "text-red-700 dark:text-red-400",
            )}
          >
            {delta}
          </span>
        )}
        {delta !== undefined && against && (
          <span className="text-muted-foreground">vs {against}</span>
        )}
      </div>

      <div className={cn("relative", named && "mt-3")}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`${label}: ${value}`}
        className="h-[88px] w-full overflow-visible"
        preserveAspectRatio="none"
      >
        <line
          x1="0"
          x2={W}
          y1={H - PAD}
          y2={H - PAD}
          className="stroke-foreground/[0.08]"
          strokeWidth="1"
          vectorEffect="non-scaling-stroke"
        />
        {variant === "bars" ? (
          shown.map((p, i) => {
            const top = y(p);
            const barWidth = Math.max(2, step * 0.55);
            return (
              <rect
                key={i}
                x={x(i) - barWidth / 2}
                y={top}
                width={barWidth}
                height={Math.max(1, H - PAD - top)}
                rx="1.5"
                className={cn(
                  "fade-in animate-in fill-mode-both duration-300",
                  i === lastIndex
                    ? "fill-blue-500 dark:fill-blue-400"
                    : "fill-foreground/25",
                )}
                style={{ animationDelay: `${i * 40}ms` }}
              />
            );
          })
        ) : (
          <>
            {variant === "area" && shown.length > 1 && (
              <path
                d={area}
                className="fill-blue-500/12 dark:fill-blue-400/15"
              />
            )}
            <polyline
              points={line}
              fill="none"
              strokeWidth="1.75"
              strokeLinecap="round"
              strokeLinejoin="round"
              vectorEffect="non-scaling-stroke"
              className="stroke-blue-500 dark:stroke-blue-400"
            />
            {last && (
              <circle
                cx={last.x}
                cy={last.y}
                r="3"
                className="fill-blue-500 dark:fill-blue-400"
              />
            )}
          </>
        )}
      </svg>
      {peak && (
        <span
          className="absolute -translate-x-1/2 -translate-y-full pb-1 text-[11.5px] font-medium whitespace-nowrap tabular-nums"
          style={{
            left: `${(x(peak.at) / W) * 100}%`,
            top: `${(y(peak.value) / H) * 88}px`,
          }}
        >
          {peak.value.toLocaleString()} · {peak.name}
        </span>
      )}
      </div>
      {named && (
        <div className="text-muted-foreground -mt-1.5 flex justify-between text-[11.5px]">
          <span>{named[0]}</span>
          <span>{named.at(-1)}</span>
        </div>
      )}
    </div>
  );
}
