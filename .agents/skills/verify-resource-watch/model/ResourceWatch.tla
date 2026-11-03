---------------------------- MODULE ResourceWatch ----------------------------
(***************************************************************************)
(* One watch registration in observeResourceScope (src/lib/resource-watch.ts) *)
(* and the backend streams it starts (start_resource_watch in               *)
(* src-tauri/crates/live/src/commands/streams.rs).                           *)
(*                                                                         *)
(* Every frontend action is one JavaScript callback, which runs to the end  *)
(* before another one starts: start() from the recovery tick, the channel   *)
(* handler for one message, the .then or .catch of startResourceWatch, and  *)
(* close() from update() or stop(). Backend actions model what the Rust     *)
(* side can do to a stream: accept or reject the start, send a status or an *)
(* error, or end the stream.                                                *)
(*                                                                         *)
(* Variant = "current" models the phase union at the sync commit.           *)
(* Variant = "pre635" models the flag-based registration before PR #635.    *)
(***************************************************************************)
EXTENDS Naturals, Sequences

CONSTANTS
    Variant,      \* "current" or "pre635"
    MaxFailures,  \* backend start rejections plus stream endings
    MaxQueue      \* undelivered non-final messages per channel

ASSUME Variant \in {"current", "pre635"}
ASSUME MaxFailures \in Nat /\ MaxQueue \in Nat

\* Each failure can force at most one restart, so this never blocks a restart.
MaxStarts == MaxFailures + 1
Starts == 1..MaxStarts

VARIABLES
    \* Registration fields of the current code (unused by pre635).
    phase,      \* "idle" | "opening" | "running" | "closed"
    \* Registration fields of the pre-#635 code (unused by current).
    closed, opening,
    \* Both versions: the channel and stream id of the watch, 0 for none.
    \* Start n opens channel n and, when accepted, stream n.
    channel, id,
    \* What the running stream last reported (WatchRegistration).
    connected, error, warning,
    \* Per start n.
    call,       \* startResourceWatch promise: "pending" | "ok" | "err" | "done"
    stream,     \* backend stream: "none" | "live" | "ended" | "refused" | "released"
    chOpen,     \* the frontend channel still delivers messages
    q,          \* messages sent on channel n and not yet handled
    starts, failures,
    stopCalls   \* starts n for which the frontend called stop_stream

vars == <<phase, closed, opening, channel, id, connected, error, warning,
          call, stream, chOpen, q, starts, failures, stopCalls>>

Current == Variant = "current"

-----------------------------------------------------------------------------
\* Views over either registration shape.

IsClosed == IF Current THEN phase = "closed" ELSE closed

\* The code records the watch as running on stream `id`.
RecordedRunning == IF Current THEN phase = "running" ELSE ~closed /\ id # 0

\* start(): the guard at the top of the function.
CanStart ==
    /\ ~warning
    /\ IF Current THEN phase = "idle" ELSE ~closed /\ ~opening /\ id = 0
    /\ starts < MaxStarts

\* closeStreamChannel(n): the channel stops delivering; queued messages drop.
CloseChannel(n) ==
    /\ chOpen' = [chOpen EXCEPT ![n] = FALSE]
    /\ q' = [q EXCEPT ![n] = <<>>]

\* stopStream(n): the backend drops the subscriber and ends the stream.
StopStream(n) ==
    /\ stream' = [stream EXCEPT ![n] = IF @ = "live" THEN "released" ELSE @]
    /\ stopCalls' = stopCalls \cup {n}

-----------------------------------------------------------------------------
\* Frontend callbacks.

\* tick(): the 15 s recovery timer calls start() for each watch, which opens
\* channel n and calls start_resource_watch. Init already holds the first
\* start(), which observeResourceScope makes before it returns.
RecoveryTick ==
    LET n == starts + 1 IN
    /\ CanStart
    /\ starts' = n
    /\ channel' = n
    /\ call' = [call EXCEPT ![n] = "pending"]
    /\ chOpen' = [chOpen EXCEPT ![n] = TRUE]
    /\ IF Current
         THEN /\ phase' = "opening" /\ UNCHANGED <<closed, opening>>
         ELSE /\ opening' = TRUE /\ UNCHANGED <<phase, closed>>
    /\ UNCHANGED <<id, connected, error, warning, stream, q, failures, stopCalls>>

\* tick() when start() returns at its guard. Nothing changes.
RecoveryTickSkips == ~CanStart /\ UNCHANGED vars

\* The channel handler created in start() handles one message from channel n.
HandlerGuard(n) ==
    IF Current THEN phase \in {"opening", "running"} /\ channel = n
               ELSE ~closed /\ channel = n

HandleStopped(n) ==
    /\ connected' = FALSE
    /\ CloseChannel(n)
    /\ IF Current
         THEN /\ phase' = "idle" /\ channel' = 0 /\ id' = 0
              /\ UNCHANGED <<closed, opening>>
         ELSE /\ id' = 0 /\ UNCHANGED <<phase, closed, opening, channel>>
    /\ UNCHANGED <<error, warning>>

HandleStatus(m) ==
    /\ warning' = (m = "forbidden")
    /\ connected' = (m = "connected")
    /\ error' = IF m \in {"connected", "forbidden"} THEN FALSE ELSE error
    /\ UNCHANGED <<phase, closed, opening, channel, id>>

HandleError ==
    /\ connected' = FALSE
    /\ error' = TRUE
    /\ UNCHANGED <<phase, closed, opening, channel, id, warning>>

Deliver(n) ==
    LET m == Head(q[n]) IN
    /\ chOpen[n] /\ q[n] # <<>>
    /\ IF ~HandlerGuard(n)
         THEN /\ q' = [q EXCEPT ![n] = Tail(@)]
              /\ UNCHANGED <<phase, closed, opening, channel, id,
                             connected, error, warning, chOpen>>
         ELSE IF m = "stopped"
           THEN HandleStopped(n)
           ELSE /\ q' = [q EXCEPT ![n] = Tail(@)]
                /\ UNCHANGED chOpen
                /\ IF m = "error" THEN HandleError ELSE HandleStatus(m)
    /\ UNCHANGED <<call, stream, starts, failures, stopCalls>>

\* .then(id) of startResourceWatch for start n.
Resolve(n) ==
    /\ call[n] = "ok"
    /\ call' = [call EXCEPT ![n] = "done"]
    /\ IF Current
         THEN /\ IF phase = "opening" /\ channel = n
                   THEN /\ phase' = "running" /\ id' = n
                        /\ UNCHANGED <<stream, stopCalls>>
                   ELSE /\ StopStream(n)
                        /\ UNCHANGED <<phase, id>>
              /\ UNCHANGED <<closed, opening>>
         ELSE /\ opening' = FALSE
              /\ IF closed
                   THEN /\ StopStream(n) /\ UNCHANGED id
                   ELSE /\ id' = n /\ UNCHANGED <<stream, stopCalls>>
              /\ UNCHANGED <<phase, closed>>
    /\ UNCHANGED <<channel, connected, error, warning, chOpen, q, starts, failures>>

\* .catch(error) of startResourceWatch for start n.
Reject(n) ==
    /\ call[n] = "err"
    /\ call' = [call EXCEPT ![n] = "done"]
    /\ IF Current
         THEN /\ IF phase = "opening" /\ channel = n
                   THEN /\ phase' = "idle" /\ channel' = 0
                        /\ error' = TRUE /\ connected' = FALSE
                        /\ CloseChannel(n)
                   ELSE UNCHANGED <<phase, channel, error, connected, chOpen, q>>
              /\ UNCHANGED <<closed, opening>>
         ELSE /\ opening' = FALSE
              /\ IF closed
                   THEN UNCHANGED <<error, connected, chOpen, q>>
                   ELSE /\ error' = TRUE /\ connected' = FALSE
                        /\ CloseChannel(n)
              /\ UNCHANGED <<phase, closed, channel>>
    /\ UNCHANGED <<id, warning, stream, starts, failures, stopCalls>>

\* close(watch) from update() dropping the key or from stop().
Close ==
    /\ ~IsClosed
    /\ IF Current
         THEN /\ phase' = "closed" /\ channel' = 0 /\ id' = 0
              /\ IF phase \in {"opening", "running"}
                   THEN CloseChannel(channel) ELSE UNCHANGED <<chOpen, q>>
              /\ IF phase = "running" THEN StopStream(id) ELSE UNCHANGED <<stream, stopCalls>>
              /\ UNCHANGED <<closed, opening>>
         ELSE /\ closed' = TRUE
              /\ IF channel # 0 THEN CloseChannel(channel) ELSE UNCHANGED <<chOpen, q>>
              /\ IF id # 0 THEN StopStream(id) ELSE UNCHANGED <<stream, stopCalls>>
              /\ UNCHANGED <<phase, opening, channel, id>>
    /\ UNCHANGED <<connected, error, warning, call, starts, failures>>

-----------------------------------------------------------------------------
\* Backend steps.

\* A message sent on a closed channel is dropped.
Send(n, m) == q' = IF chOpen[n] THEN [q EXCEPT ![n] = Append(@, m)] ELSE q

\* start_resource_watch subscribes, spawns run_resource_watch, returns Ok(id).
Accept(n) ==
    /\ call[n] = "pending" /\ stream[n] = "none"
    /\ call' = [call EXCEPT ![n] = "ok"]
    /\ stream' = [stream EXCEPT ![n] = "live"]
    /\ UNCHANGED <<phase, closed, opening, channel, id, connected, error, warning,
                   chOpen, q, starts, failures, stopCalls>>

\* start_resource_watch returns Err before it subscribes.
Refuse(n) ==
    /\ call[n] = "pending" /\ stream[n] = "none"
    /\ failures < MaxFailures
    /\ call' = [call EXCEPT ![n] = "err"]
    /\ stream' = [stream EXCEPT ![n] = "refused"]
    /\ failures' = failures + 1
    /\ UNCHANGED <<phase, closed, opening, channel, id, connected, error, warning,
                   chOpen, q, starts, stopCalls>>

\* run_resource_watch reports connected, reconnecting, or a transient error.
Report(n) ==
    /\ stream[n] = "live"
    /\ chOpen[n] => Len(q[n]) < MaxQueue
    /\ \E m \in {"connected", "reconnecting", "error"} : Send(n, m)
    /\ UNCHANGED <<phase, closed, opening, channel, id, connected, error, warning,
                   call, stream, chOpen, starts, failures, stopCalls>>

\* The stream ends with its final message: "forbidden" (report_forbidden_watch),
\* "error" (invalid kind or API resource), or "stopped" (the StreamMessage
\* contract; the Rust resource watch does not send it today).
EndStream(n) ==
    /\ stream[n] = "live"
    /\ failures < MaxFailures
    /\ stream' = [stream EXCEPT ![n] = "ended"]
    /\ failures' = failures + 1
    /\ \E m \in {"forbidden", "error", "stopped"} : Send(n, m)
    /\ UNCHANGED <<phase, closed, opening, channel, id, connected, error, warning,
                   call, chOpen, starts, stopCalls>>

-----------------------------------------------------------------------------

Init ==
    \* observeResourceScope calls start() for each key before it returns.
    /\ phase = IF Current THEN "opening" ELSE "unused"
    /\ closed = FALSE
    /\ opening = ~Current
    /\ channel = 1
    /\ id = 0
    /\ connected = FALSE /\ error = FALSE /\ warning = FALSE
    /\ call = [n \in Starts |-> IF n = 1 THEN "pending" ELSE "done"]
    /\ stream = [n \in Starts |-> "none"]
    /\ chOpen = [n \in Starts |-> n = 1]
    /\ q = [n \in Starts |-> <<>>]
    /\ starts = 1
    /\ failures = 0
    /\ stopCalls = {}

Next ==
    \/ RecoveryTick
    \/ RecoveryTickSkips
    \/ Close
    \/ \E n \in Starts :
         \/ Deliver(n) \/ Resolve(n) \/ Reject(n)
         \/ Accept(n) \/ Refuse(n) \/ Report(n) \/ EndStream(n)

\* Timers fire, queued messages arrive, the backend answers each start call,
\* and promises settle. Backend failures and Close happen only if they happen.
Fairness ==
    /\ WF_vars(RecoveryTick)
    /\ \A n \in Starts :
         /\ WF_vars(Deliver(n))
         /\ WF_vars(Resolve(n))
         /\ WF_vars(Reject(n))
         /\ WF_vars(Accept(n) \/ Refuse(n))

Spec == Init /\ [][Next]_vars /\ Fairness

-----------------------------------------------------------------------------
\* The property.

\* The watch's newest stream ended or its start was refused, while the
\* watch is still open.
StreamLost == ~IsClosed /\ stream[starts] \in {"ended", "refused"}

\* The user is told, or the watch runs again on a live stream.
Recovered ==
    \/ IsClosed
    \/ warning
    \/ RecordedRunning /\ (stream[id] = "live" \/ error)

\* Safety: the code never records the watch as running on a stream that has
\* ended and has no messages left, unless it shows a warning or an error.
NoSilentDeadStream ==
    RecordedRunning /\ stream[id] # "live" /\ q[id] = <<>> => warning \/ error

\* Liveness: a lost stream is always followed by recovery or a report.
LostStreamRecovers == StreamLost ~> Recovered

\* Model check: the start bound never blocks a restart the code would make.
StartBoundRoom ==
    (IF Current THEN phase = "idle" ELSE ~closed /\ ~opening /\ id = 0)
        /\ ~warning => starts < MaxStarts

TypeOK ==
    /\ phase \in {"idle", "opening", "running", "closed", "unused"}
    /\ closed \in BOOLEAN /\ opening \in BOOLEAN
    /\ channel \in 0..MaxStarts /\ id \in 0..MaxStarts
    /\ connected \in BOOLEAN /\ error \in BOOLEAN /\ warning \in BOOLEAN
    /\ starts \in Starts /\ failures \in 0..MaxFailures
    /\ stopCalls \subseteq Starts
=============================================================================
