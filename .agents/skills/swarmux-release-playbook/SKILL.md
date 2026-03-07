---
name: swarmux-release-playbook
description: Execute and verify GitHub Releases for this repo. Use when the user asks to cut a release, publish artifacts, trigger release workflow, or check why a release did not publish.
---

# Release Playbook

Run only the minimum commands needed to publish a release.

## Release Now

1. Trigger release on `main`.
```bash
gh workflow run Release --ref main
```
2. Watch latest run.
```bash
gh run list --workflow Release --limit 1
gh run watch <run-id> --exit-status
```
3. If run opened/updated release PR but no artifacts were published:
```bash
gh pr list --state open --search "release in:title"
```
Merge that release PR, then trigger release workflow again:
```bash
gh workflow run Release --ref main
```

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

Always report:
- run URL
- whether a PR was created/updated
- release tag published
- assets uploaded
