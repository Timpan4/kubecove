# Svelte Topology Spike Results

Status: harness added; measurements below are from `bun --expose-gc scripts/perf-topology-compare.ts` on 2026-06-14.

Decision rule: continue toward Svelte only if renderer RAM or CPU-heavy interactions improve by at least 25% without topology UX loss.

## Manual Testing

Run `bun run dev`, then open:

- React: `http://localhost:1430/src/features/resources/topology-spike/react.html?nodes=1000`
- Svelte: `http://localhost:1430/src/features/resources/topology-spike/svelte.html?nodes=1000`

Change `nodes` to `4000` or `10000` for heavier graphs. Add `&autorun=1` to trigger the benchmark harness from the page.

## Bundle Impact

| Framework | Raw | Gzip | Largest asset |
| --- | ---: | ---: | --- |
| React | 0.38 MiB | 0.12 MiB | `assets/react-1Z-fhdHC.js` (0.36 MiB raw) |
| Svelte | 0.22 MiB | 0.07 MiB | `assets/svelte-CbBLHBPj.js` (0.21 MiB raw) |

Svelte's isolated topology bundle is about 41% smaller raw and 39% smaller gzip in this harness.

## Browser Metrics

| Nodes | React total | Svelte total | Delta | React heap after | Svelte heap after |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | n/a | n/a | n/a | n/a | n/a |
| 4,000 | n/a | n/a | n/a | n/a | n/a |
| 10,000 | n/a | n/a | n/a | n/a | n/a |

Browser automation did not run because this machine has no local Chromium, Chrome, or Edge channel available to `playwright-core`. Manual browser testing or installing a Playwright browser is still needed before applying the 25% RAM/CPU decision rule.

## Shared Layout Cost

These numbers exercise the shared synthetic graph generator and current topology layout helper before either renderer mounts.

| Nodes | Edges | Layout | Selections | RSS | Heap |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1,000 | 800 | 16.73 ms | 100 | 196.52 MiB | 26.51 MiB |
| 4,000 | 3,200 | 42.19 ms | 100 | 223.17 MiB | 26.51 MiB |
| 10,000 | 8,000 | 104.93 ms | 100 | 296.69 MiB | 26.51 MiB |

## Recommendation

Do not migrate yet. The spike shows useful bundle-size upside, but the required renderer RAM and CPU interaction data is not available from automation on this machine. Keep React unless manual/browser-backed results show Svelte clearing the 25% bar with topology UX parity.

Notes:

- This spike compares `@xyflow/react` and `@xyflow/svelte` using the same synthetic KubeCove topology data and the existing topology layout helper.
- The harness intentionally stays out of the product route, so normal KubeCove builds do not ship the Svelte topology page.
- Svelte Flow 1.6.0 is marked alpha by its package README; any migration decision should include that maturity risk.
