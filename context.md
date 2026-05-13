# Code Context

## Files Retrieved

1. `src/App.tsx` (lines 1-280) - Root component, Zustand store consumer, ArgoCD detect effect
2. `src/lib/hooks.ts` (lines 1-200) - Zustand store definitions and composition hook
3. `src/components/SidebarTree.tsx` (lines 1-280) - Sidebar component, does NOT subscribe to useDashboardState
4. `src/components/ClusterSelector.tsx` (lines 1-70) - Cluster selector, does NOT subscribe to useDashboardState
5. `src/components/ResourceList.tsx` (lines 1-100, 240-300, 460-560) - Resource list with row click handler

## Key Code

### Zustand Store Architecture (`src/lib/hooks.ts:59-108`)

```typescript
// Two SEPARATE stores - critical for understanding re-renders
const usePersistedStore = create<PersistedSlice>()(
  persist(
    (set) => ({
      clusterContext: "", // <-- Only changes on cluster switch
      // ...
    }),
    { name: "k8s-manager-dashboard" },
  ),
);

const useNonPersistedStore = create<NonPersistedSlice>()((set) => ({
  selectedResource: null, // <-- Changes on row click
  // ...
}));

// Combined hook
export function useDashboardState() {
  const persisted = usePersistedStore(); // Separate subscription
  const nonPersisted = useNonPersistedStore(); // Separate subscription
  return { ...persisted, ...nonPersisted };
}
```

### App's ArgoCD Detect Effect (`src/App.tsx:90-106`)

```typescript
useEffect(() => {
  if (!clusterContext) {
    setArgoDetected(false);
    return;
  }
  // ... async detectArgoCD call
}, [clusterContext, setArgoDetected]); // Only depends on clusterContext
```

### handleResourceSelect (`src/App.tsx:72-76`)

```typescript
const handleResourceSelect = useCallback(
  (resource: ResourceSummary) => {
    setSelectedResource(resource); // Only updates useNonPersistedStore
  },
  [setSelectedResource], // Stable - Zustand setters have stable identity
);
```

### ResourceList Row Click (`src/components/ResourceList.tsx:516-522`)

```typescript
onClick={() => {
  setSelectedResourceKey(resourceKey);  // LOCAL state - does NOT trigger App re-render
  onResourceSelect(row.original);  // Calls handleResourceSelect
}}
```

## Architecture

1. **Two separate Zustand stores**: `usePersistedStore` (clusterContext) and `useNonPersistedStore` (selectedResource). These are independent subscriptions.

2. **App subscribes to both** via `useDashboardState()`, but each store update only triggers subscribers of THAT store.

3. **Click flow**:
   - User clicks row → `onResourceSelect(row.original)`
   - `handleResourceSelect` calls `setSelectedResource(resource)`
   - `setSelectedResource` only updates `useNonPersistedStore`
   - `usePersistedStore` subscribers (including App's dependency on `clusterContext`) are NOT notified

4. **ArgoCD detect effect**: Only depends on `[clusterContext, setArgoDetected]`. `clusterContext` is in `usePersistedStore` and does NOT change when `setSelectedResource` is called.

## Start Here

- `src/lib/hooks.ts` - Understand the dual-store architecture
- `src/App.tsx:90-106` - The ArgoCD detect effect
- `src/components/ResourceList.tsx:516-522` - Where the click originates

## Supervisor Coordination

**Critical finding**: Based on code analysis, there is NO mechanism that would cause the ArgoCD detect effect to fire twice when a pod row is clicked.

- `setSelectedResource` updates only `useNonPersistedStore`
- `clusterContext` is in `usePersistedStore` (separate store)
- App's effect depends only on `[clusterContext, setArgoDetected]`
- Neither dependency changes on row click

**Possible causes of double-fire**:

1. React StrictMode in development (double-invokes effects by design)
2. Something else in the call stack not visible in this code
3. A second `setSelectedResource` call somewhere

To diagnose further: Check if the app is wrapped in `<React.StrictMode>` and whether the double-fire happens in dev only or also prod.
