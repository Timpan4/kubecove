# SOLID Kubernetes Phabricator Workflow

## Summary

Use Dynamist Phabricator workflow as the primary work process for SOLID Kubernetes.

One board only: `SOLID Kubernetes`.

Phabricator owns:

- demand
- priority
- owner
- workflow state
- acceptance

Git owns:

- architecture decisions
- implementation recipes
- code and configuration changes
- validation evidence

Use `phabfive` for Phabricator automation. All Phabricator access must be scoped to `SOLID Kubernetes`.

## Work Items

Use lightweight hierarchy. Do not force `[EPIC] -> [US] -> Task`.

| Work size     | Phabricator shape                                   | Time box            | Definition                                                                                   |
| ------------- | --------------------------------------------------- | ------------------- | -------------------------------------------------------------------------------------------- |
| Tiny          | no separate ticket if discovered during active work | minutes to hours    | Trivial typo, one-line fix, tiny local cleanup, or harmless correction inside touched scope. |
| Small         | plain Task                                          | up to 2-3 work days | One clear outcome, one branch/MR, no architecture decision, no cross-component dependency.   |
| Medium        | `[US]` or parent Task                               | up to 1-2 work weeks | Several steps, multiple touched areas, acceptance criteria needed, maybe blueprint.          |
| Large         | `[EPIC]`                                            | multiple weeks      | Multiple outcomes or phases, several MRs, likely ADR/BP set, coordination needed.            |
| Decision      | plain Task                                          | up to 2-3 work days | Goal is a decision. Output may be an ADR.                                                    |
| Investigation | plain Task                                          | up to 2-3 work days | Goal is to learn or confirm. Output is a Phab comment, maybe follow-up work.                 |

Default to plain Task for planned work. Split or promote work when it no longer
fits the time box. Use `[US]` when work needs acceptance criteria and breakdown.
Use `[EPIC]` only for multi-phase coordination.

## Ticket Creation

Every ticket must define a concise, measurable Definition of Done before it can
move to `Up next`.

Ticket Definition of Done rules:

- State the expected output or behavior.
- State how completion will be verified.
- Use measurable checks, commands, links, or observable states.
- Avoid vague terms such as "improve", "clean up", "fix", or "make better"
  unless the ticket also defines the exact accepted result.
- If no one can tell whether the ticket is done from the ticket text, rewrite
  the ticket before work starts.

Good examples:

- `kubectl kustomize is/system/environments/dev` exits 0 after the change.
- MR adds `Ref T1234` and updates the ArgoCD ApplicationSet template.
- ArgoCD app `keycloak` is Synced and Healthy in `dev`.
- ADR status is changed to `Accepted` and the decision date is set.

Bad examples:

- "Fix ArgoCD."
- "Clean up manifests."
- "Improve docs."

## Board Flow

Board columns:

`Backlog -> Up next -> In progress -> In review -> Done`

Column meaning:

- `Backlog`: captured, not necessarily ready.
- `Up next`: clear, testable, feasible, accepted for near-term work.
- `In progress`: assignee is actively working.
- `In review`: MR or other output is ready for review.
- `Done`: Definition of Done is met.

Rules:

- Never set Phabricator ticket status to `Resolved`.
- Completion during daily work means moving the card to `Done` only after the
  Definition of Done is met.
- `Done` tickets are resolved later by humans during a recurring sit-down, roughly every two weeks.
- Phabricator status is not real-time workflow state. Board column is workflow truth.
- Tickets move from `Backlog` to `Up next` only when a user asks.
- Users or AI agents may move cards, but agents must not promote backlog work on their own.

## Definition of Done

A ticket may move to `Done` only when all applicable checks are complete.

Universal checks:

- Scope is complete, or the requested decision/investigation output is posted.
- Required MR(s) are merged, or no-MR output is posted on the ticket.
- Review comments are handled or explicitly deferred with owner agreement.
- Scope-matched validation has passed.
- Validation evidence is posted in the MR and, when useful, on the Phabricator ticket.
- Discovered work is fixed, split, deferred, or ignored according to the Discovered Work rules.
- Final Phabricator comment links the MR or output and states validation result,
  deployment/cluster status, and follow-up work if any.
- When work uses multiple MRs, each MR meets its own stated Definition of Done
  before the ticket moves to `Done`.

Infra/platform checks:

- Relevant ArgoCD, cluster, OpenTofu, or platform state is healthy for the ticket scope.
- Any accepted degraded state or deferred rollout is written on the ticket before moving to `Done`.

Code/docs-only checks:

- MR is merged.
- Requested review is complete.
- No cluster health check is required unless the change affects deployed behavior.

Decision/investigation checks:

- Decision, finding, or recommendation is posted on the ticket.
- Follow-up tasks are created only when they need priority, owner, acceptance, or future tracking.

## Traceability

If a Phabricator ticket exists, all delivery artifacts must reference it.

Branch names:

- Must include task ID.
- Example: `feat/T1234-argocd-bootstrap`.

Commit messages:

- Subject stays outcome-focused.
- Body/footer references task ID with `Ref T1234`.
- Use `Ref`, not `Fixes`, `Closes`, or `Resolves`, because agents must not auto-close Phabricator tickets.
- For multiple tickets, use one `Ref` line per ticket.

Example:

```text
fix(system): correct ArgoCD route backend

Ref T1234
```

Merge requests:

- MR title must start with the primary task ID.
- MR title stays outcome-focused after the ticket prefix.
- MR body must reference task ID with `Ref T1234`.
- MR body must state the relevant Definition of Done when the MR does not cover
  the full ticket scope.
- For multiple tickets, use one `Ref` line per ticket in the MR body.
- If discovered work gets a separate MR but no new ticket, use the current/source ticket ID.
- If work truly has no ticket, MR body must explain why no ticket exists.

Example:

```text
[T1234] fix(system): correct ArgoCD route backend
```

## Repo Workflow

Phabricator is the source of demand and workflow state. Repo artifacts support execution and evidence.

- Phab ticket says what to do and why.
- ADR records durable architecture decisions.
- Blueprint records execution recipe for complex or repeatable work.
- MR records actual change.
- Validation evidence belongs in both MR and Phab comment when useful.
- `docs/logs/bp-*` and Phab comments complement each other:
    - repo log keeps durable technical execution trace;
    - Phab comment keeps board-facing status and evidence.
- ADR statuses should be cleaned up so accepted decisions are marked `Accepted`.

## MR Scope

Prefer small, reviewable MRs. Split work into separate MRs when one MR would
make review slow, noisy, or risky.

Split MRs when:

- the ticket has independent implementation steps;
- changes span unrelated components;
- generated or mechanical changes would obscure hand-written changes;
- validation or rollout must happen in stages;
- a discovered fix deserves independent review or history.

Each MR must state what part of the ticket it completes and which Definition of
Done applies to that MR. The Phabricator ticket moves to `Done` only after all
required MRs and ticket-level checks are complete.

## Blueprint Rule

Blueprints are optional by default.

Require a blueprint when:

- work has ordered infra/platform steps where sequence matters;
- work has operator run order or stateful infra risk;
- work spans multiple components or MRs;
- work must be repeatable by agents or humans later;
- rollback, troubleshooting, or validation is non-trivial;
- work changes platform bootstrapping or environment layout.

Do not require a blueprint for:

- tiny or small fixes;
- isolated bugs;
- straightforward config changes;
- documentation-only changes;
- investigations whose output is only a finding.

If a ticket needs a blueprint, create or update it as part of the work. Phabricator still owns priority and acceptance.

## Discovered Work

Avoid ticket churn. Not every discovered issue needs a Phab task.

When bug or cleanup appears during other work:

| Case                                  | Action                                                                                                                   |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Tiny and inside touched scope         | Fix in same MR. Mention only if useful.                                                                                  |
| Blocks current task                   | Fix in same MR. Mention in current ticket/MR because it explains scope.                                                  |
| Needed for validation                 | Fix in same MR if narrow. If larger, create separate MR and link from current MR/ticket.                                 |
| Same touched area, low risk           | Fix in same MR if clearly part of current outcome. Otherwise create separate MR; no separate ticket required.            |
| Related, self-contained, non-blocking | Create separate MR with clear title/body. No Phab task required unless tracking is useful. Use source ticket ID.         |
| Related and needs priority/acceptance | Create or link plain Task on `SOLID Kubernetes`. Do not fix now unless user asks.                                        |
| Unrelated but tiny                    | Create separate MR only if worth doing now and user agrees. Otherwise note only.                                         |
| Unrelated and non-tiny                | Create Task if worth tracking. Otherwise note only. Do not fix.                                                          |
| Security, data loss, or secrets risk  | Stop, warn, create urgent Task on `SOLID Kubernetes`; fix only with explicit approval unless blocking safe continuation. |

Principles:

- Ticket needed when work needs priority, owner, acceptance, or future tracking.
- Separate MR needed when change deserves independent review/history.
- Same MR is fine when change is necessary or truly local to current task.
- No ticket or MR needed for things noticed but not worth doing.

## Agent / phabfive Behavior

For implementation work, agent starts from a Phabricator ticket.

1. Read current ticket with `phabfive`.
2. Verify ticket belongs to `SOLID Kubernetes`; otherwise stop.
3. Do not access other Phabricator projects unless user explicitly permits it.
4. Scope all search/list operations to `SOLID Kubernetes`.
5. Check task is clear, testable, and feasible before implementation.
6. If the ticket Definition of Done is vague or not measurable, ask for or
   propose a concrete DoD before work starts.
7. Read linked ADR/BP only when ticket references it or work needs it.
8. Move card to `In progress` only when work starts.
