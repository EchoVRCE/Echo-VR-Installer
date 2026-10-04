---
name: start-sprint
description: Start any evr-launcher sprint by preserving repository state, assigning an item branch or worktree, and opening visible work through early commits and a draft PR. Run before the first sprint item.
---

# Start a sprint

Run this skill before any sprint work. `AGENTS.md` names the current integration branch and owns the shared verification and handoff gates. Report what you checked, including the commands, populations covered, and gaps in coverage; a bare "clean" or "nothing pending" is not enough.

## Own the assignment

At assignment start, set a Codex Goal for the full assigned outcome, with checkable completion evidence, scope, and constraints. Use `/goal` or the Goal tool yourself; do not wait for another person to restate or nudge the next action. A Goal persists in this Codex thread across turns, so continue until the evidence meets its completion condition. See [official OpenAI Goals documentation](https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex).

After each named assignment step or handoff, report the artifact, commit SHA, and actual verification to the assigned recipient by courier. Check the courier inbox before the next step and incorporate new instructions. If the courier destination is missing, report that gap rather than inventing an address. An agent that stalls is replaced, not coaxed: preserve its work, fix the missing route in `AGENTS.md`, and start a fresh agent.

## Inventory and preserve the repository

Before creating a branch, editing, cleaning, or deciding what work is safe to take:

1. Enumerate every registered worktree with `git worktree list --porcelain`; record path, HEAD, branch, and any detached HEAD.
2. Run `git status --porcelain=v1 --untracked-files=all` in each worktree. Record staged, unstaged, and untracked paths. Do not disturb dirty work, including work whose owner is absent. Preserve it on its current branch before considering cleanup or reuse; if it must move, first make a reversible WIP commit on that branch with its unverified status stated. Never discard it to make the inventory look clean.
3. Inventory local and remote branches with `git branch -vv --all` and stashes with `git stash list`. For each local branch, identify its upstream or state that none exists; compare both directions with that upstream to find unpushed commits, and compare with the integration branch to find unmerged commits. Inspect the actual commit subjects with `git log --oneline <base>..<branch>` before classifying a branch. A branch without a worktree still counts.
4. Report the inventory method and coverage: which worktrees, branches, upstreams, and stash namespace were examined; which commits are unpushed or unmerged; which dirty or detached states were preserved; and any ref or remote that could not be checked. Do not turn a missing or unresolved ref into a "clean" verdict.

Never use `git add -A`, `rm -rf`, or a `--force` option for sprint hygiene. Stage only explicit owned paths. Do not reset, stash, check out, or overwrite another contributor's work.

## Give each item its own visible history

Use one task branch or worktree per sprint item, based on the integration branch in `AGENTS.md`. If an item already has its own branch and PR, continue them after the inventory; do not create a duplicate. Keep unrelated items out of its commits and PR.

Commit each coherent logical change as soon as it is ready. Each commit message says why the change exists; include the verification actually run. Do not hold all changes for one end-of-sprint commit. Stage explicit paths and inspect the index before committing. Do not use a closing keyword in a commit message for a partially completed issue.

Push the item branch after its first coherent commit and keep a PR open from that commit onward. Use a draft PR until the gates in `AGENTS.md` pass; update it as work and evidence arrive. Push only the intended branch by explicit refspec. A checked result belongs to the commit that was checked, so rerun affected checks after material fixes before claiming the PR is ready.

Write the PR description for a reader unfamiliar with the agents: what changed, why, how to review or verify it, and what is outside its scope. Link design and check evidence where applicable. Use product and code terms rather than agent names or workflow jargon. Target the integration branch in `AGENTS.md`.
