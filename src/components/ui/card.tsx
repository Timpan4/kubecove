import { cva, type VariantProps } from "class-variance-authority"
import type * as React from "react"

import { cnfast } from "@/lib/utils"

export const cardVariants = cva(
  "group/card flex flex-col gap-4 overflow-hidden rounded-lg bg-surface-1 py-4 text-xs/relaxed text-card-foreground border border-border has-[>img:first-child]:pt-0 data-[size=sm]:gap-3 data-[size=sm]:py-3 *:[img:first-child]:rounded-t-lg *:[img:last-child]:rounded-b-lg transition-shadow duration-150",
  {
    variants: {
      elevation: {
        flat: "shadow-none",
        raised: "shadow-sm hover:shadow-md hover:border-primary/30",
        overlay:
          "shadow-xl border-border/60 bg-surface-2 backdrop-blur-2xl backdrop-saturate-150 z-popover",
      },
    },
    defaultVariants: {
      elevation: "flat",
    },
  }
)

type CardElevation = NonNullable<VariantProps<typeof cardVariants>["elevation"]>

function Card({
  className,
  size = "default",
  elevation = "flat",
  ...props
}: React.ComponentProps<"div"> & {
  size?: "default" | "sm"
  elevation?: CardElevation
}) {
  return (
    <div
      data-slot="card"
      data-size={size}
      className={cnfast(cardVariants({ elevation }), className)}
      {...props}
    />
  )
}

function CardHeader({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-header"
      className={cnfast(
        "group/card-header @container/card-header grid auto-rows-min items-start gap-1 rounded-t-lg px-4 group-data-[size=sm]/card:px-3 has-data-[slot=card-action]:grid-cols-[1fr_auto] has-data-[slot=card-description]:grid-rows-[auto_auto] [.border-b]:pb-4 group-data-[size=sm]/card:[.border-b]:pb-3",
        className
      )}
      {...props}
    />
  )
}

function CardTitle({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-title"
      className={cnfast("font-heading text-sm font-medium", className)}
      {...props}
    />
  )
}

function CardDescription({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-description"
      className={cnfast("text-xs/relaxed text-muted-foreground", className)}
      {...props}
    />
  )
}

function CardAction({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-action"
      className={cnfast(
        "col-start-2 row-span-2 row-start-1 self-start justify-self-end",
        className
      )}
      {...props}
    />
  )
}

function CardContent({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-content"
      className={cnfast("px-4 group-data-[size=sm]/card:px-3", className)}
      {...props}
    />
  )
}

function CardFooter({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="card-footer"
      className={cnfast(
        "flex items-center rounded-b-lg px-4 group-data-[size=sm]/card:px-3 [.border-t]:pt-4 group-data-[size=sm]/card:[.border-t]:pt-3",
        className
      )}
      {...props}
    />
  )
}

export {
  Card,
  CardHeader,
  CardFooter,
  CardTitle,
  CardAction,
  CardDescription,
  CardContent,
}
