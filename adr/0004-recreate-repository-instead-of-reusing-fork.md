# ADR-0004 — Delete the upstream fork and recreate a clean repository

Date: 2026-08-22 · Status: accepted (executed at milestone M0)

## Decision
`ABCrimson/ModernSeasonvarDownloader` currently exists as an untouched GitHub fork of `DoITCreative/SeasonvarDownloader`. It will be deleted and a clean, non-fork public repository created with the same name; the rewrite is pushed there as `main`. Attribution to the original moves to the README (and this ADR), not to git history.

## Why
The owner's explicit choice: the rewrite shares no code or language with the upstream (clean-room from independently re-verified protocol facts), and a fork relationship would misrepresent it (PR targets, "forked from" badge, fork-network semantics, inability to restore if deleted later).

## Rejected
- **Reuse the fork, rewrite on `main`:** keeps provenance and the badge with zero destructive action — the recommended option, declined by the owner.
- **New repo under a different name:** keeps the fork around as dead weight; name is already right.

## Consequence
Irreversible: GitHub cannot restore a deleted repository that was part of a fork network. Mitigated by (a) the fork holding only upstream commits, (b) executing the deletion only at M0 after the local first commit exists, (c) requiring the owner to run `gh auth refresh -h github.com -s delete_repo` themselves (the token lacks the scope), which is the human confirmation step.

## Deliberately unresolved
- Whether to open an issue/PR on the upstream pointing to the rewrite (default: a polite README mention only).
