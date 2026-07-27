# Issue Tracker: GitHub

Issues and PRDs for this repo live as GitHub Issues in `Timpan4/kubecove`. Use the `gh` CLI for issue-tracker operations.

## Conventions

- **Create an issue**: `gh issue create --repo Timpan4/kubecove --title "..." --body "..."`. Use a heredoc for multi-line bodies.
- **Read an issue**: `gh issue view <number> --repo Timpan4/kubecove --comments`, filtering comments by `jq` and also fetching labels when needed.
- **List issues**: `gh issue list --repo Timpan4/kubecove --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'` with appropriate `--label` and `--state` filters.
- **Comment on an issue**: `gh issue comment <number> --repo Timpan4/kubecove --body "..."`
- **Apply or remove labels**: `gh issue edit <number> --repo Timpan4/kubecove --add-label "..."` or `gh issue edit <number> --repo Timpan4/kubecove --remove-label "..."`
- **Close an issue**: `gh issue close <number> --repo Timpan4/kubecove --comment "..."`

Always pass `--repo Timpan4/kubecove` so commands target the canonical issue tracker, even from forks or clones with missing remotes.

## When a skill says "publish to the issue tracker"

Create a GitHub issue.

## When a skill says "fetch the relevant ticket"

Run `gh issue view <number> --repo Timpan4/kubecove --comments`.
