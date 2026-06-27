# Design System

The visual language is shared through one token layer in `src/App.css`, so design refinements land once and flow through the Svelte UI.

## Stack

- **Tailwind v4** (CSS-first config via `@theme inline`, no `tailwind.config.*`).
- **bits-ui** + hand-authored shadcn-svelte-style wrappers in `src/components/ui/svelte/`.
- **OKLCH** colors throughout. Dark mode is always on (`.dark` added at boot in `main-svelte.ts`); the `:root` light palette is declared but unused in production.
- Fonts: **Inter Variable** (`--font-sans`, body), **Geist Variable** (`--font-heading`, titles), **Geist Mono Variable** (`--font-mono`, code/metrics/kbd). Loaded via `@fontsource-variable`.

## Token layers (all in `src/App.css`)

### Color — `:root` + `.dark`
`--background`, `--foreground`, `--card`, `--popover`, `--primary`, `--secondary`, `--muted`, `--accent`, `--destructive`, `--border`, `--input`, `--ring`, `--sidebar-*`, `--chart-1..5`, `--resource-*` (per k8s kind tone). The `.dark` values are what ship.

### Surface steps
`--surface-0/1/2/3` — subtle bg lifts off `--card` via `color-mix`. Use `bg-surface-0/1/2/3` for layered depth (e.g. hero bands vs cards vs raised stat cards). Never reach for raw `bg-card` when a surface token expresses the intent.

### Elevation (shadows)
`--shadow-xs/sm/md/lg/xl/2xl` — soft OKLCH-tinted shadows (dark theme uses near-black with opacity, never pure black). Exposed as Tailwind `shadow-*` utilities. Prefer `shadow-sm`/`shadow-md` for cards and `shadow-xl`/`shadow-2xl` for floating surfaces (dialogs, popovers).

### Stacking
`--z-base/content/sticky/overlay/popover/toast/dialog` — single source of truth for `z-*` utilities. Do not hardcode `z-50`/`z-40`; use `z-popover`, `z-dialog`, `z-toast`, etc.

### Radius
`--radius` (base, 0.625rem) derived into `--radius-sm/md/lg/xl/2xl/3xl/4xl`. Controls use `rounded-md`, surfaces `rounded-lg`, pills `rounded-full`.

### Typography scale
`--text-display/xl/lg` mapped to Tailwind `text-display` etc. Body baseline stays `text-xs` (12px); titles opt into `text-sm`/`text-lg`/`text-xl`/`text-display`. Use `.tabular-nums` (or `font-mono`) for metric/age/counter values.

## Primitive conventions

- Every primitive uses `cn()` from `@/lib/utils` and `data-slot="..."` attributes (shadcn convention).
- Buttons default to `h-7 text-xs`; sizes `xs/sm/default/lg/icon/icon-xs/icon-sm/icon-lg`. Density is intentionally compact — do not loosen without reason.
- `Card` has an `elevation?: "flat" | "raised" | "overlay"` prop (both runtimes). `flat` = resting, `raised` = hoverable list card (shadow + hover lift), `overlay` = floating surface (blur + strong depth). Use `raised` for list cards, `flat` for forms/inline, `overlay` for command palettes.
- Focus style everywhere: `focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30`.
- Invalid style: `aria-invalid:border-destructive ... aria-invalid:ring-destructive/20`.

## Motion

- Finite transitions only (`transition-* duration-150`). Hover-lift on buttons/cards via `hover:shadow-sm`/`hover:shadow-md`.
- **No infinite animations** inside transformed/scaled containers — WebKit (macOS/Linux) intermittently rasterizes animated layers blank (see topology glow fix in `App.css`). Infinite animations that exist (`foreground-loading-bar`, `topology-edge-flow`) live outside transformed subtrees and all respect `prefers-reduced-motion`.

## Cross-platform verification

Shadow and blur layers must be checked on WebKit (macOS) and WebKitGTK (Linux), not just WebView2 (Windows). Local `cargo clippy`/`bun` checks only validate your OS. If you add elevation/blur to a transformed container, call it out in the PR.
