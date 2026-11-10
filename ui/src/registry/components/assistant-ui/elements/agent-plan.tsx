"use client";

import type { ComponentProps } from "react";
import {
  CircleAlertIcon,
  CircleCheckIcon,
  LoaderIcon,
  SquareIcon,
  SquareMinusIcon,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { mono } from "./surfaces";
import { announced, pct, progressOf } from "../utils/range";

export type AgentPlanStatus =
  | "done"
  | "active"
  | "pending"
  | "blocked"
  | "failed"
  | "stopped";

export type AgentPlanStep = {
  id?: string | undefined;
  label: string;
  description?: string | undefined;
  /** The step's own state; without it, its place against activeIndex decides. */
  status?: AgentPlanStatus | undefined;
  /** Who the step is for, at the row's end. */
  owner?: string | undefined;
};

const spoken: Record<AgentPlanStatus, string> = {
  done: "done",
  active: "in progress",
  pending: "not started",
  blocked: "blocked",
  failed: "failed",
  stopped: "stopped",
};

const outcome: Partial<Record<AgentPlanStatus, string>> = {
  blocked: "Blocked",
  failed: "Failed",
  stopped: "Stopped",
};

export function AgentPlan({
  steps,
  activeIndex,
  title = "Plan",
  header = true,
  className,
  ...props
}: Omit<ComponentProps<"div">, "children" | "steps" | "activeIndex"> & {
  steps: readonly (string | AgentPlanStep)[];
  activeIndex: number;
  title?: string | undefined;
  /** The title and progress above the steps; off when a docked bar shows them. */
  header?: boolean | undefined;
}) {
  const total = steps.length;
  const completed = progressOf(activeIndex, total);
  const allDone = completed >= total;
  const progress = pct(completed, total);

  return (
    <div
      data-slot="agent-plan"
      className={cn("flex w-full max-w-sm flex-col gap-3", className)}

      {...props}
    >
      {header ? (
        <>
          <div className="flex items-center justify-between">
            <span className="text-[13.5px] font-medium">{title}</span>
            <span className={cn(mono, "text-muted-foreground tabular-nums")}>
              {completed} of {total}
            </span>
          </div>
          <div
            role="progressbar"
            aria-label={`${title} progress`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={announced(progress)}
            aria-valuetext={`${completed} of ${total} steps`}
            className="bg-foreground/[0.06] inset-ring-border h-[3px] w-full overflow-hidden rounded-full inset-ring forced-colors:outline"
          >
            <span
              aria-hidden
              className="bg-foreground/80 block h-full rounded-full transition-[width] duration-500 forced-color-adjust-none motion-reduce:transition-none"
              style={{ width: `${progress}%` }}
            />
          </div>
        </>
      ) : null}
      <ul className="flex flex-col">
        {steps.map((step, i) => {
          const item = typeof step === "string" ? { label: step } : step;
          const status: AgentPlanStatus =
            (typeof step === "string" ? undefined : step.status) ??
            (allDone || i < completed
              ? "done"
              : i === completed
                ? "active"
                : "pending");
          const active = status === "active";
          const word = outcome[status];
          const detail = (active || word) && item.description;
          return (
            <li
              key={typeof step === "string" ? i : (step.id ?? i)}
              data-status={status}
              className="flex items-start gap-2.5 py-[5px] text-[13px]"
            >
              <span className="flex h-5 w-4 shrink-0 items-center justify-center">
                {status === "done" ? (
                  <CircleCheckIcon aria-hidden className="text-foreground size-3.5" />
                ) : active ? (
                  <LoaderIcon
                    aria-hidden
                    className="size-3 animate-spin text-[var(--attention)] [animation-duration:1.2s] motion-reduce:animate-none"
                  />
                ) : status === "blocked" ? (
                  <SquareMinusIcon aria-hidden className="text-muted-foreground size-3.5" />
                ) : status === "failed" ? (
                  <CircleAlertIcon aria-hidden className="text-destructive size-3.5" />
                ) : status === "stopped" ? (
                  <SquareIcon aria-hidden className="text-muted-foreground size-3.5" />
                ) : (
                  <span
                    aria-hidden
                    className="text-muted-foreground size-[7px] rounded-full border border-current"
                  />
                )}
              </span>
              <span className="flex min-w-0 flex-1 flex-col gap-px">
                <span
                  className={cn(
                    "leading-5",
                    active ? "text-foreground" : "text-muted-foreground",
                  )}
                >
                  {item.label}
                  <span className="sr-only">{`, ${spoken[status]}`}</span>
                </span>
                {word ? (
                  <span
                    aria-hidden
                    className={cn(
                      "text-xs leading-normal font-medium",
                      status === "failed"
                        ? "text-destructive"
                        : "text-muted-foreground",
                    )}
                  >
                    {word}
                  </span>
                ) : null}
                {detail ? (
                  <span className="text-muted-foreground text-xs leading-normal">
                    {item.description}
                  </span>
                ) : null}
              </span>
              {typeof step !== "string" && step.owner ? (
                <span className="text-muted-foreground shrink-0 text-[11px] leading-5">
                  {step.owner}
                </span>
              ) : null}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
