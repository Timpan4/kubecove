# Design System

Svelte UI shares one token layer in `src/App.css`.

## Stack

- Tailwind v4 CSS-first configuration through `@theme inline`.
- bits-ui and hand-authored wrappers in `src/components/ui/svelte/`.
- OKLCH colors. Production starts dark mode in `main-svelte.ts`.
- Inter Variable for body, Geist Variable for headings, Geist Mono Variable for code and metrics.

## Tokens

All tokens live in `src/App.css`.

- Color: semantic `--background`, `--foreground`, `--card`, `--primary`, `--muted`, `--destructive`, `--sidebar-*`, `--chart-*`, and `--resource-*` tokens.
- Surface: `--surface-0/1/2/3` for layered depth; use `bg-surface-*` when intent is depth.
- Elevation: `--shadow-*`; use modest shadows for cards and stronger shadows only for floating surfaces.
- Stacking: `--z-*`; do not hardcode `z-50` or `z-40`.
- Radius and type: use defined `rounded-*` and `text-*` scales; use `.tabular-nums` or `font-mono` for metrics.

## Svelte Primitive Rules

- Primitives use `cn()` from `@/lib/utils` and `data-slot`.
- Buttons default compact. Keep density unless interaction needs more space.
- Use `Card` elevation: `flat` for forms, `raised` for hoverable lists, `overlay` for floating surfaces.
- Preserve visible keyboard focus and `aria-invalid` styles.

## Motion and Platforms

Use finite transitions. Infinite animation in transformed or scaled containers can rasterize blank in WebKit; existing infinite effects remain outside transformed subtrees and respect `prefers-reduced-motion`.

Check shadow and blur changes on WebKit and WebKitGTK as well as WebView2. Call out transformed elevation or blur in PR.
