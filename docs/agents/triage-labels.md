# Triage Labels

The skills speak in terms of five canonical triage roles. This file maps those roles to the target label strings for this repo's issue tracker.

| Canonical role      | Label in our tracker | Meaning                                  |
| ------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | Maintainer needs to evaluate this issue  |
| `needs-info`               | `needs-info`         | Waiting on reporter for more information |
| `ready-for-agent`          | `ready-for-agent`    | Fully specified, ready for an AFK agent  |
| `ready-for-human`          | `ready-for-human`    | Requires human implementation            |
| `wontfix`                  | `wontfix`            | Will not be actioned                     |

When a skill mentions a role, use the corresponding label string from this table.

This setup records the label vocabulary only; it does not create labels in GitHub. Before applying a label, verify it exists with `gh label list --repo Timpan4/kubecove`. If a configured label is missing, create that exact label or update this mapping before applying it.
