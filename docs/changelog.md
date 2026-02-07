# Changelog

## When to write one

Create a changelog entry for any version bump. If it's worth a new version, it's worth documenting why.

## Where

One file per version in `changelog/`, named by tag: `changelog/v0.6.0.md`.

## Format

```markdown
# vX.Y.Z

One-line theme.

## Why

1-3 sentences on what motivated this release.

## Changes

### Category name
- **Change title** — what changed and why.

\`\`\`rust
// Before/after snippet for the most impactful changes
\`\`\`
```

## Guidelines

- **Document motivations, not diffs.** The git log already says what files changed. The changelog should say why the change was made — what problem it solves, what was wrong before, what trade-off was chosen.
- **Categories are freeform.** Group by theme (e.g., "Concurrency", "Auth", "Binary integrity"), not by file or module. Each release tells its own story.
- **Keep it scannable.** Bold the change title, follow with a dash and the motivation. One line per change unless the reasoning is non-obvious.
- **Include code for key changes.** Add before/after snippets for the most impactful changes — the ones where seeing the code makes the motivation click. Skip snippets for trivial or self-explanatory changes.
- **No backfill.** Changelogs start at v0.6.0. Prior history lives in git.
