---
name: verify-resource-watch
description: Formally check KubeCove's resource-watch lifecycle (idle, opening, running, closed in src/lib/resource-watch.ts) with TLA+ and TLC, then replay the model against the real observeResourceScope. Use when a change touches resource-watch.ts, the stream wrappers in src/lib/tauri-streams.ts, the StreamMessage or StreamStatus contract, or how the Rust resource watch starts, reports, or ends a stream.
---

# Verify the resource-watch lifecycle

## The property

A watch whose stream has ended is eventually running on a new live stream, or shows a permission warning or an error. It is never left recorded as running on a dead stream with nothing shown.

The harm when it breaks: a resource view keeps showing "Reconnecting; periodically reloading this view" with no error, the recovery tick never restarts the watch, and live updates for that kind stop for good. PR #635 fixed one such path: a `stopped` message that arrived before `start_resource_watch` resolved left the watch recorded as running.

The model checks it as two rules in `model/ResourceWatch.tla`:

- `NoSilentDeadStream` (invariant): the code never records the watch as running on a stream that has ended and has no messages left to deliver, unless it shows a warning or an error.
- `LostStreamRecovers` (liveness): when the watch's newest stream ends or its start is refused, the watch is later closed, shows a permission warning, or is recorded as running on a live stream or with an error shown.

## Run the check

From the repository root:

```sh
bun .agents/skills/verify-resource-watch/scripts/check.ts
```

It runs in a few seconds and prints one `PASS` or `FAIL` line per check. TLC runs in a work folder under the OS temp directory. A pass deletes it. A failure keeps it and prints its path, because it holds each TLC output and the state graph:

1. TLC on the model of the current code: all rules hold, with no deadlock.
2. TLC on the pre-#635 variant must still break `NoSilentDeadStream`.
3. TLC on the pre-#635 variant must still break `LostStreamRecovers`.
4. The replay: every edge of TLC's state graph for the current model, replayed against the real `observeResourceScope` in `src/lib/resource-watch.ts`. Edges after a close are replayed twice, once through `update([])` and once through `stop()`.

Checks 2 and 3 prove the rules can still fail. The replay proves the model describes the code. It compares the code's observable state with the model after every step: the number of `start_resource_watch` calls, which channels are open, which streams got `stop_stream`, and whether the published state shows an error, a permission warning, or connected.

Tools: TLC 1.7.4 (`tla2tools.jar`) and Temurin Java 25, pinned in `scripts/check.ts` with the same mise specs the statechart skill uses. If they are missing, the script prints the `mise install` command. Ask the user before you install them. `TLA2TOOLS` and `JAVA` override the lookup. The replay needs the repository's `node_modules` (`bun install`).

## Does my change touch the verified code?

Coverage at sync commit `ca62604` (origin/main, KubeCove v0.13.0):

| Code | What the model takes from it |
| --- | --- |
| `src/lib/resource-watch.ts`: `observeResourceScope`, `start`, the channel handler inside `start`, the `.then` and `.catch` of `startResourceWatch`, `tick`, `close`, `update` when it drops a key, `stop` | Every frontend action, one action per callback |
| `src/lib/resource-watch.ts`: `publish` | Only the published `error`, `warnings`, and `status` |
| `src/lib/tauri-streams.ts`: `startResourceWatch`, `stopStream`, `closeStreamChannel` | A closed channel delivers nothing more |
| `src/lib/types.ts`: `StreamMessage`, `StreamStatus` | The messages `status` (connected, reconnecting, forbidden), `error`, `stopped` |
| `src-tauri/crates/live/src/commands/streams.rs`: `start_resource_watch`, `stop_stream` | The start is refused before it subscribes, or it subscribes and returns the stream id |
| `src-tauri/crates/live/src/commands/streams/watch.rs`: `run_resource_watch`, `report_forbidden_watch` | A stream ends after `forbidden` or a final `error`; transient errors and `reconnecting` keep it live |
| `src-tauri/crates/live/src/commands/streams/registry.rs`: `stop` | `stop_stream` ends the stream |

Run `git diff ca62604 -- <those paths>`. If the diff changes any listed function, run the check. Move the sync commit and this table forward only after you have confirmed that the model describes the code at the new commit.

## Read a failure

- **Check 1 fails, check 4 was skipped.** TLC found a path that breaks a rule. Its output file has the trace, one state per step, with the action name on each step. Tell it as a story in the code's words, such as "start 1 opens, the stream sends `stopped`, the handler runs, then `.then` records the watch as running". Then decide:
  - If you changed the model in this change, the model may be wrong. Compare each action you touched with its code.
  - If you changed only code and check 4 passed before, the code now has the bug. Report it with the path.
- **Checks 2 or 3 fail.** The pre-#635 variant no longer breaks a rule, so a rule or an action got weaker. Fix the model. Never weaken a rule to make check 1 pass.
- **Check 4 fails.** The code and the model disagree. The output lists the steps that led there and each observable that differs, such as `stop_stream calls: code [], model [1]`. Then decide:
  - If the code change was not meant to change behavior, the code has a bug. The listed steps reproduce it.
  - If the change was intended, update the model action that models that callback, and run the check again. If check 1 then fails, the intended behavior breaks the property. That is a real finding, so report it and leave the property as it is.
- **Check 4 says the state graph cannot be replayed.** TLC's state graph has no edges, has edges to states it does not define, or has states the initial state cannot reach. A new TLC version or a `TLA2TOOLS` override may write a DOT format that `readStateGraph` in `scripts/replay.ts` does not parse. Fix the parser; this says nothing about the code or the model.

## Update the verification for an intended change

1. Edit the action in `model/ResourceWatch.tla` that models the changed callback. Keep one action per JavaScript callback and use the code's names. A new callback gets a new action, a new case in `stepOf` and `run` in `scripts/replay.ts`, and a fairness condition when the runtime always eventually runs it.
2. A new observable field needs a model variable and a comparison in `differences` in `scripts/replay.ts`.
3. Model the pre-#635 code only in its `Variant = "pre635"` branches. They must keep breaking both rules.
4. Run the check until all four lines pass. Then move the sync commit and update the coverage table.

## Bounds and what the model leaves out

The configs set `MaxFailures = 2` (refused starts plus stream endings in one run) and `MaxQueue = 1` (non-final messages waiting per channel). TLC explores 1,192 states. A pass covers these bounds only.

The model leaves out:

- More than one key. Each key has its own registration, but observers share `needsCatchUp`, reloads, and `scopeGeneration`, which are not modeled.
- `refresh`, `reload`, `catchingUp`, reload warnings, and `readError`.
- `update()` adding keys. Only dropping a key (close) is modeled.
- One backend watch shared by several subscribers in the registry, and its replay of the last status to a new subscriber.
- The delay between calling `stop_stream` and the backend ending the stream. The model ends it at once; the channel is already closed in each case.
- `resourceChanged` messages. The channel handler passes them to `onChange`, and they change no lifecycle state.
- Two guards the replay does not exercise. Deleting either one still passes the check, so a change to them needs its own test:
  - The channel handler's stale-channel check, `watch.phase.channel !== channel`. The model never delivers on a closed channel, as the `tauri-streams.ts` row assumes.
  - The `watch.permissionWarning` check at the top of `start`. No state the model reaches has an idle watch with a warning.
- The Rust resource watch does not send `stopped` today. Only `logs.rs` does. The model still lets a stream end with `stopped`, because the `StreamMessage` contract and the browser mocks allow it, and the #635 bug lives on that path.

An error counts as reported. After a final `error`, the watch stays recorded as running and is not restarted, while the status reads "Reconnecting; periodically reloading this view" next to the error. The property accepts that, because the error is shown.

## Draw the model as a statechart

If the `statechart` skill is installed, it draws the model and numbers each bug path in the reader's words with `model/view.ts`:

```sh
<statechart skill>/scripts/statechart .agents/skills/verify-resource-watch/model/ResourceWatch.tla --config .agents/skills/verify-resource-watch/model/Pre635Safety.cfg --view .agents/skills/verify-resource-watch/model/view.ts --out <new folder outside the repo>
```

At `MaxFailures = 2` the model has more states than the chart's budget of 500, so the chart shows only the bug paths. To draw the whole graph, pass a copy of a config with `MaxFailures = 1`.
