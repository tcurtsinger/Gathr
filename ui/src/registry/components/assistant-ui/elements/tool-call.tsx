"use client";

import { CheckIcon, ChevronRightIcon, MinusIcon, XIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import {
  CollapsibleRoot as Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "./collapsible-root";
import {
  collapsePanel,
  field,
  mono,
  ShimmerLabel,
  SwapLabel,
} from "./surfaces";

export interface ToolCallProps {
  label: string;
  activeLabel: string;
  query: string;
  request: string;
  result: string;
  running: boolean;
  failed?: boolean;
  /** Gathr: cut off before it ended (the owner's call, room task 143): a dash and "Stopped", never a check. */
  stopped?: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  className?: string;
}

export function ToolCall({
  label,
  activeLabel,
  query,
  request,
  result,
  running,
  failed = false,
  stopped = false,
  open,
  onOpenChange,
  className,
}: ToolCallProps) {
  return (
    <Collapsible
      data-slot="tool-call"
      open={open}
      onOpenChange={onOpenChange}
      className={cn("w-full max-w-sm", className)}
    >
      <CollapsibleTrigger className="group/trigger text-muted-foreground hover:text-foreground/90 flex w-full items-center gap-2 rounded-md py-1 text-[13.5px] transition-colors outline-none">
        <ChevronRightIcon className="size-3.5 shrink-0 opacity-60 transition-transform duration-200 ease-[cubic-bezier(0.32,0.72,0,1)] group-data-open/trigger:rotate-90 group-data-panel-open/trigger:rotate-90 motion-reduce:transition-none" />
        <SwapLabel
          active={running ? 0 : 1}
          className="shrink-0 text-start whitespace-nowrap"
        >
          <ShimmerLabel
            active={running}
            className="relative inline-block leading-none"
          >
            {activeLabel}
          </ShimmerLabel>
          <>{label}</>
        </SwapLabel>
        <span
          className={cn(
            mono,
            "bg-foreground/[0.06] text-foreground/70 min-w-0 truncate rounded-md px-1.5 py-0.5",
          )}
        >
          {query}
        </span>
        {/* Gathr: its own marks (room task 143): done neutral, failed in the destructive colour. */}
        <span className="ms-auto flex min-w-4 shrink-0 items-center justify-end gap-1">
          {running ? null : failed ? (
            <XIcon
              role="img"
              aria-label="Failed"
              className="fade-in zoom-in-90 animate-in size-3.5 text-destructive duration-200"
            />
          ) : stopped ? (
            <>
              Stopped
              <MinusIcon aria-hidden="true" className="size-3.5" />
            </>
          ) : (
            <CheckIcon className="fade-in zoom-in-90 animate-in size-3.5 duration-200" />
          )}
        </span>
      </CollapsibleTrigger>
      <CollapsibleContent className={cn(collapsePanel, "outline-none")}>
        <div className={cn(field, "mt-2 overflow-hidden rounded-2xl text-xs")}>
          <div className="px-3.5 pt-2.5 pb-2">
            <p className={cn(mono, "text-muted-foreground mb-1")}>Request</p>
            <p className="text-muted-foreground font-mono break-words">
              {request}
            </p>
          </div>
          <div className="bg-foreground/[0.06] mx-3.5 h-px" />
          <div className="px-3.5 pt-2 pb-2.5">
            <p className={cn(mono, "text-muted-foreground mb-1")}>Result</p>
            <p className="text-foreground/90 break-words">{result}</p>
          </div>
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}
