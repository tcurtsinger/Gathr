"use client";

import type { ComponentProps } from "react";
import { BotIcon, UserIcon, WrenchIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { mono } from "./surfaces";

export type SpeakerKind = "user" | "agent" | "subagent" | "tool";

export interface SpeakerTurn {
  id: string;
  kind: SpeakerKind;
  name: string;
  detail?: string;
  text: string;
}

const TONE: Record<SpeakerKind, string> = {
  user: "bg-foreground/[0.06] text-muted-foreground",
  agent: "bg-foreground/[0.06] text-foreground text-[11px] font-medium",
  subagent: "bg-foreground/[0.06] text-muted-foreground",
  tool: "bg-foreground/[0.04] text-muted-foreground",
};

// Gathr: an agent shows its initials, not the shared robot icon (DESIGN.md).
function initials(name: string) {
  const words = name.split(/\s+/).filter(Boolean);
  return words.slice(0, 2).map((word) => [...word][0]?.toUpperCase()).join("");
}

export function SpeakerIdentity({
  turns,
  className,
  ...props
}: Omit<ComponentProps<"div">, "children" | "turns"> & {
  turns: readonly SpeakerTurn[];
}) {
  return (
    <div
      data-slot="speaker-identity"
      className={cn("flex w-full max-w-sm flex-col gap-3.5", className)}

      {...props}
    >
      {turns.map((turn) => (
        <div key={turn.id} className="grid grid-cols-[1.5rem_minmax(0,1fr)] gap-x-2.5 gap-y-0.5">
          <span
            className={cn(
              "col-start-1 row-start-1 flex size-6 shrink-0 items-center justify-center self-center rounded-lg",
              TONE[turn.kind],
              (turn.kind === "subagent" || turn.kind === "agent") && "rounded-full",
            )}
          >
            {turn.kind === "user" ? (
              <UserIcon className="size-3" />
            ) : turn.kind === "tool" ? (
              <WrenchIcon className="size-3" />
            ) : turn.kind === "agent" ? (
              initials(turn.name)
            ) : (
              <BotIcon className="size-3" />
            )}
          </span>

            <span className="col-start-2 row-start-1 flex min-w-0 items-baseline gap-1.5 self-center">
              <span className="text-[13px] font-medium">{turn.name}</span>
              {turn.detail && (
                <span className={cn(mono, "text-muted-foreground")}>
                  {turn.detail}
                </span>
              )}
            </span>
            {turn.text && <span className="col-start-2 row-start-2 text-foreground/65 text-[13.5px] leading-relaxed break-words">
              {turn.text}
            </span>}
        </div>
      ))}
    </div>
  );
}
