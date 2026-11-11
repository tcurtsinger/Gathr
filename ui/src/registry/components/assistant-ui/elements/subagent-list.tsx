"use client";

import type { ComponentProps, ReactNode } from "react";
import { CheckIcon, Loader2Icon } from "lucide-react";
import { cn } from "@/lib/utils";
import { mono, paper } from "./surfaces";
import { pct } from "../utils/range";
import {
  CollapsibleContent,
  CollapsibleRoot,
  CollapsibleTrigger,
} from "./collapsible-root";

export interface SubagentItem {
  name: string;
  model: string;
  /** Inline rows: in place of the status icon, such as the agent's initials. */
  avatar?: ReactNode;
  /** Inline rows: the line under the name, in place of the model. */
  detail?: ReactNode;
  /** Inline rows: the count after the progress bar, such as 1/2; without it, no bar. */
  count?: string | undefined;
}

export function SubagentList({
  agents,
  completedCount,
  progress,
  showSummary,
  summaryAgent,
  presentation = "card",
  expandedAgent,
  onExpandedAgentChange,
  renderAgentTasks,
  renderAgentActions,
  className,
  ...props
}: Omit<
  ComponentProps<"div">,
  | "children"
  | "agents"
  | "completedCount"
  | "progress"
  | "showSummary"
  | "summaryAgent"
> & {
  agents: readonly SubagentItem[];
  completedCount: number;
  progress: readonly number[];
  showSummary: boolean;
  summaryAgent: SubagentItem;
  /** inline: rows without cards, each opening its tasks below it. */
  presentation?: "card" | "inline" | undefined;
  expandedAgent?: string | undefined;
  onExpandedAgentChange?: ((name: string | undefined) => void) | undefined;
  renderAgentTasks?: ((agent: SubagentItem) => ReactNode) | undefined;
  /** At the row's top right, outside its toggle, such as a menu. */
  renderAgentActions?: ((agent: SubagentItem) => ReactNode) | undefined;
}) {
  if (presentation === "inline") {
    return (
      <div
        data-slot="subagent-list"
        className={cn("flex w-full flex-col", className)}
        {...props}
      >
        {agents.map((agent, index) => {
          const percentage = pct(progress[index] ?? 0, 100);
          const row = (
            <>
              <span className="flex items-center gap-2.5 pr-7">
                {agent.avatar}
                <span className="flex min-w-0 flex-1 flex-col leading-[1.4]">
                  <span className="truncate pr-(--subagent-actions,0px) text-[13px] font-medium">
                    {agent.name}
                  </span>
                  <span className="text-muted-foreground text-[11px]">
                    {agent.detail ?? agent.model}
                  </span>
                </span>
              </span>
              {agent.count ? (
              <span className="flex items-center gap-2 pl-[34px]">
                <span
                  role="progressbar"
                  aria-label={`${agent.name} progress`}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(percentage)}
                  aria-valuetext={agent.count}
                  className="bg-foreground/[0.06] h-[3px] flex-1 overflow-hidden rounded-full forced-colors:outline"
                >
                  <span
                    className="bg-foreground/80 block h-full rounded-full transition-[width] duration-500 forced-color-adjust-none motion-reduce:transition-none"
                    style={{ width: `${percentage}%` }}
                  />
                </span>
                <span className={cn(mono, "text-muted-foreground tabular-nums")}>
                  {agent.count}
                </span>
              </span>
              ) : null}
            </>
          );
          const rowClass =
            "flex w-full flex-col gap-1.5 rounded-[10px] p-2 text-start hover:bg-foreground/[0.07]";
          return (
            <div
              key={agent.name}
              data-slot="subagent-row"
              className="relative py-0.5"
            >
              {onExpandedAgentChange && renderAgentTasks ? (
                <CollapsibleRoot
                  open={expandedAgent === agent.name}
                  onOpenChange={(open) =>
                    onExpandedAgentChange(open ? agent.name : undefined)
                  }
                >
                  <CollapsibleTrigger className={rowClass}>{row}</CollapsibleTrigger>
                  <CollapsibleContent>{renderAgentTasks(agent)}</CollapsibleContent>
                </CollapsibleRoot>
              ) : (
                <div className={rowClass}>{row}</div>
              )}
              {renderAgentActions ? (
                <span className="absolute top-2.5 right-1">
                  {renderAgentActions(agent)}
                </span>
              ) : null}
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <div
      data-slot="subagent-list"
      className={cn("flex w-full max-w-xs flex-col gap-2", className)}

      {...props}
    >
      {agents.map((agent, index) => {
        const done = index < completedCount;
        const width = progress[index] ?? 0;
        const percentage = pct(width, 100);

        return (
          <div
            key={agent.name}
            className={cn(
              paper,
              "flex flex-col gap-2 rounded-2xl px-3.5 py-2.5",
            )}
          >
            <div className="flex items-center gap-2">
              {done ? (
                <CheckIcon className="fade-in zoom-in-90 animate-in size-3.5 shrink-0 text-emerald-500 duration-200" />
              ) : (
                <Loader2Icon className="text-muted-foreground size-3.5 shrink-0 animate-spin motion-reduce:animate-none" />
              )}
              <span className="flex-1 truncate text-[13.5px]">
                {agent.name}
              </span>
              <span className={cn(mono, "text-muted-foreground")}>
                {agent.model}
              </span>
            </div>
            <span
              role="progressbar"
              aria-label={`${agent.name} progress`}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={percentage}
              className="bg-foreground/[0.06] inset-ring-border h-[3px] w-full overflow-hidden rounded-full inset-ring forced-colors:outline"
            >
              <span
                className={cn(
                  "block h-full rounded-full transition-[width] duration-700 forced-color-adjust-none",
                  done ? "bg-emerald-500/70" : "bg-foreground/60",
                )}
                style={{ width: `${percentage}%` }}
              />
            </span>
          </div>
        );
      })}
      {showSummary && (
        <div
          className={cn(
            paper,
            "fade-in slide-in-from-bottom-2 animate-in flex flex-col gap-2 rounded-2xl px-3.5 py-2.5 duration-300",
          )}
        >
          <div className="flex items-center gap-2">
            <Loader2Icon className="text-muted-foreground size-3.5 shrink-0 animate-spin motion-reduce:animate-none" />
            <span className="flex-1 truncate text-[13.5px]">
              {summaryAgent.name}
            </span>
            <span className={cn(mono, "text-muted-foreground")}>
              {summaryAgent.model}
            </span>
          </div>
          <span
            role="progressbar"
            aria-label={`${summaryAgent.name} progress`}
            aria-valuemin={0}
            aria-valuemax={100}
            className="bg-foreground/[0.06] inset-ring-border h-[3px] w-full overflow-hidden rounded-full inset-ring forced-colors:outline"
          >
            <span className="shimmer shimmer-bg block h-full w-full rounded-full motion-reduce:animate-none" />
          </span>
        </div>
      )}
    </div>
  );
}
