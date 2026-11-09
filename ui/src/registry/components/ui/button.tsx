import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { cn } from "cn"
import { Slot } from "radix-ui"
import { field, ghostButton, inkButton } from "../assistant-ui/elements/surfaces"

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-2 rounded-full text-xs font-medium whitespace-nowrap outline-none " +
  "focus-visible:ring-1 focus-visible:ring-foreground/20 disabled:pointer-events-none disabled:opacity-40 " +
  "aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 " +
  "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: inkButton,
        destructive:
          "bg-red-500/10 text-red-500 hover:bg-red-500/15 transition-colors duration-150 active:scale-[0.96]",
        outline:
          [ghostButton, "border border-border text-foreground"],
        secondary:
          [ghostButton, field, "text-foreground"],
        ghost:
          ghostButton,
        link: "text-primary underline-offset-4 hover:underline",
      },
      size: {
        default: "h-8 px-3.5",
        xs: "h-6 gap-1 px-2 text-xs has-[>svg]:px-1.5 [&_svg:not([class*='size-'])]:size-3",
        sm: "h-8 gap-1.5 px-3.5",
        lg: "h-10 px-6 has-[>svg]:px-4",
        icon: "size-9",
        "icon-xs": "size-6 [&_svg:not([class*='size-'])]:size-3",
        "icon-sm": "size-8",
        "icon-lg": "size-10",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

function Button({
  className,
  variant = "default",
  size = "default",
  asChild = false,
  ...props
}: React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & {
    asChild?: boolean
  }) {
  const Comp = asChild ? Slot.Root : "button"

  return (
    <Comp
      data-slot="button"
      data-variant={variant}
      data-size={size}
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  )
}

export { Button, buttonVariants }
