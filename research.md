# Research: React.memo, useEffect deps, and Zustand object stability

## Summary

`React.memo` uses shallow comparison (Object.is semantics). A new object literal passed as a prop always breaks memoization — the child re-renders. `useEffect` deps compare object references the same way; `useMemo` is the fix. Zustand's `set()` always creates a new store state object, but selector equality is what triggers re-renders; use `useShallow` or stable selectors to avoid unnecessary renders.

---

## Findings

### 1. React.memo does shallow prop comparison — new object reference always triggers re-render

**Yes, React.memo does shallow comparison using `Object.is` against each prop.** This is not deep value comparison. If a parent creates a new object on every render — even with identical primitive values — `React.memo` sees it as a changed prop and re-renders the child.

```jsx
// Parent re-renders → new object every time → child ALWAYS re-renders
function Parent() {
  return (
    <Child
      resource={{ name: "x", namespace: "y", cluster: "z", kind: "Pod" }}
    />
  );
}

// FIX: stabilize with useMemo so the same reference is passed
function Parent() {
  const resource = useMemo(
    () => ({ name: "x", namespace: "y", cluster: "z", kind: "Pod" }),
    [],
  );
  return <Child resource={resource} />;
}
```

Evidence:

- Official React docs state `memo` compares props "with `Object.is`" (reference equality), not structural equality. [react.dev/reference/react/memo](https://react.dev/reference/react/memo)
- "My component re-renders when a prop is an object, array, or function" — React docs explicitly calls out this pitfall. [react.dev/reference/react/memo](https://react.dev/reference/react/memo)
- The official pattern: "minimize the times that the props change. For example, if the prop is an object, prevent the parent component from re-creating that object every time by using useMemo." [react.dev/reference/react/memo](https://react.dev/reference/react/memo)

**If the parent re-renders and creates `resource = {name: "x", ...}` fresh each time, the child wrapped in `memo` WILL re-render.** Shallow equality always fails for `{} !== {}`.

---

### 2. Correct pattern: prevent useEffect from firing when object ref changes but primitives don't

**Use `useMemo` in the parent OR destructure primitives as deps.** `useEffect` deps use the same `Object.is` semantics — reference changes fire the effect regardless of deep equality.

**Pattern A — stabilize the object with useMemo:**

```jsx
// Object doesn't change reference → effect fires only on real change
function Component({ resource }) {
  useEffect(() => {
    // runs once when resource reference changes
  }, [resource]); // resource is stable from parent
}

// Parent passes stable reference
function Parent() {
  const resource = useMemo(() => ({ name: "x", namespace: "y" }), []);
  return <Component resource={resource} />;
}
```

**Pattern B — destructure primitives as individual deps:**

```jsx
// Effect reacts to individual primitive values, not the object reference
function Component({ name, namespace }) {
  useEffect(() => {
    // runs only when name or namespace actually changes
  }, [name, namespace]);
}
```

**Pattern C — create object inside the Effect (React docs recommended):**

```jsx
// Move object creation inside the Effect to avoid reference dependency
function Component({ name, namespace }) {
  useEffect(() => {
    const options = { name, namespace }; // created fresh inside → stable
    // use options here
  }, [name, namespace]); // deps are primitives → only fires on actual change
}
```

Evidence: React docs explicitly show this pattern — "Avoid using an object created during rendering as a dependency. Instead, create the object inside the Effect." [react.dev/learn/removing-effect-dependencies](https://react.dev/learn/removing-effect-dependencies)

**What does NOT work:**

- `useEffect(..., [obj.prop1, obj.prop2])` — if `obj` is a new reference, `obj.prop1` and `obj.prop2` come from that new object, so the effect fires
- `useEffect(..., [arrVar])` with `setArrVar([1,2,3])` — comparing array references always fails; `[1,2,3] !== [1,2,3]`

---

### 3. Does Zustand's `set({ selectedResource: resource })` create new state every time?

**Yes, `set()` always produces a new state object reference.** However, whether this causes re-renders depends on selector equality, not on `set()` itself.

**Key behavior:**

- `set(state => ({...state, selectedResource: resource}))` always creates a new top-level state object
- If the selected resource is the same reference (by `Object.is`), the component won't re-render — Zustand compares selector output with previous
- If `set()` is called with the same value (same reference), Zustand may bail out, but the store state object itself is still new

**The real re-render trigger: selectors returning new objects**

```jsx
// PROBLEM: selector returns new object every render
const { selected } = useStore((state) => ({
  selected: state.selectedResource, // new object every call!
}));
// Every store change → new selector output → re-render

// FIX with useShallow
const { selected } = useStore(
  useShallow((state) => ({ selected: state.selectedResource })),
);
// Shallow comparison of the object → only re-renders when properties actually change

// FIX with primitive selector
const selected = useStore((state) => state.selectedResource);
// Primitive reference is stable if the value hasn't changed
```

Evidence from official Zustand docs: "Zustand uses strict equality (`===`) by default to compare the previous and next selected value. If the selector returns a new object or array on every call, the component re-renders on every state change because `{} !== {}`." [reactdevelopers.org/docs/zustand/selectors](https://reactdevelopers.org/docs/zustand/selectors)

**From Zustand's own issue tracker** (GitHub Discussion #3228), the same pattern confirmed: a selector returning a new object every render (`(state) => ({ activeItem: state.props.a })`) causes infinite re-renders. The fix is to select primitives or use `useShallow`. [github.com/pmndrs/zustand/discussions/3228](https://github.com/pmndrs/zustand/discussions/3228)

---

## Sources

- Kept: [React.memo docs](https://react.dev/reference/react/memo) — official source, confirms shallow comparison and useMemo fix
- Kept: [Removing Effect Dependencies](https://react.dev/learn/removing-effect-dependencies) — official docs, shows the object-inside-Effect pattern
- Kept: [useEffect reference](https://react.dev/reference/react/useEffect) — confirms Object.is for deps comparison
- Kept: [Zustand Selectors & Performance](https://reactdevelopers.org/docs/zustand/selectors) — definitive guide on selector equality
- Kept: [useShallow docs](https://www.mintlify.com/pmndrs/zustand/api/use-shallow) — correct usage for multi-value selection
- Kept: [GitHub Discussion #3228](https://github.com/pmndrs/zustand/discussions/3228) — real-world case of selector causing infinite re-renders
- Dropped: Generic Stack Overflow answers without official source backing (kept only answers citing React/Zustand docs)

---

## Gaps

- No specific performance benchmark data on shallow vs deep comparison overhead for large objects
- `useSyncExternalStore` internals in Zustand not fully explored — the underlying cause of the re-render loop in Discussion #3228 referenced React internals without confirming the exact code path
- React Compiler (automatic memo) — not tested since this project uses React 18 and doesn't have the compiler enabled; could eliminate manual `memo` in future

---

## Practical Patterns for this Project

For K8s-manager resource selection state:

```tsx
// Good: primitive selector — stable, no unnecessary re-renders
const resource = useResourceStore((s) => s.selectedResource);

// If you need multiple fields:
const { name, kind } = useResourceStore(
  useShallow((s) => ({
    name: s.selectedResource?.name,
    kind: s.selectedResource?.kind,
  })),
);

// In store, avoid creating new objects for unchanged values:
set((state) => {
  // Only spread and create new state object when value actually differs
  if (state.selectedResource === resource) return state; // bail out
  return { ...state, selectedResource: resource };
});
```
