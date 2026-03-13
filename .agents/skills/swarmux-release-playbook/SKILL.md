---
name: swarmux-release-playbook
description: Execute and verify GitHub Releases for this repo. Use when the user asks to cut a release, publish artifacts, trigger release workflow, or check why a release did not publish.
---

# Release Playbook

Run only the minimum commands needed to publish a release.

## Preflight First

Before triggering anything, inspect three states:

1. Latest published tag.
```bash
gh release list --limit 1
```
2. Version on `main`.
```bash
gh api repos/$(gh repo view --json nameWithOwner -q .nameWithOwner)/contents/Cargo.toml?ref=main --jq .content | base64 -d | sed -n '1,20p'
```
3. Open release PRs.
```bash
gh pr list --state open --search "release in:title"
```

Interpretation:
- If `main`'s `Cargo.toml` version is behind the latest published tag, `main` has not caught up to already-published state yet.
- If an open release PR version is less than or equal to the latest published tag, that PR is stale/catch-up only. Merging it will not publish a new release; it only advances `main` to already-published state.
- Only expect a publish after `main` reflects the latest published version and the next release PR targets a strictly newer version.

## Release Now

1. Run preflight first.
2. If there is an open release PR whose version is less than or equal to the latest published tag, merge it first. Repeat preflight until any remaining open release PR targets a version greater than the latest published tag.
3. If there is an open release PR whose version is greater than the latest published tag, merge that PR, then trigger release on `main`.
```bash
gh workflow run Release --ref main
```
4. If there is no open release PR and `main`'s version equals the latest published tag, trigger release on `main` to let `release-pr` open the next release PR. Then merge that PR and trigger release again.
5. Watch latest run.
```bash
gh run list --workflow Release --limit 1
gh run watch <run-id> --exit-status
```
6. If run opened/updated release PR but no artifacts were published:
```bash
gh pr list --state open --search "release in:title"
```
Merge that release PR, then trigger release workflow again:
```bash
gh workflow run Release --ref main
```
7. If the post-merge run still publishes nothing and the `release` job log shows `Already published - Tag ... already exists`, the merged release PR was stale/catch-up. Merge the newly opened release PR and trigger `Release` again until a new tag is published.

## Verify Published Release

```bash
gh release list --limit 5
gh release view <tag> --json tagName,name,publishedAt,url,assets
```

## Inspect Why It Skipped

```bash
gh run view <run-id> --json jobs
gh run view <run-id> --job <job-id> --log
```

Look for:
- `release_output: {"releases":[]}` means no publish happened.
- `dist-*` jobs `skipped` means no release tag output from `release`.
- `Already published - Tag ... already exists` means the merged release PR targeted a version that was already released; expect a newer release PR to be created.
- A release page can appear before assets upload finishes; only call assets uploaded after `upload-release` succeeds.

Always report:
- run URL
- whether a PR was created/updated
- release tag published
- assets uploaded
