import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cnfast(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export { cnfast as cn }

/**
 * Normalize raw status strings from Kubernetes/Helm ("deployed",
 * "pending-install") into badge labels ("Deployed", "Pending install") so
 * status casing is consistent across product areas.
 */
export function formatStatusLabel(value: string): string {
  const label = value.replace(/-/g, " ")
  return label.charAt(0).toUpperCase() + label.slice(1)
}
