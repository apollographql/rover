# Changesets

Each pull request that makes a user-visible change adds one file to this directory describing it. When a release is cut, the files are assembled into that release's section of [`CHANGELOG.md`](../CHANGELOG.md) and its GitHub release notes, then deleted.

One file per change means pull requests never edit the same lines, so changelog entries don't conflict.

## Adding a changeset

```sh
mise run add-changeset
```

It asks for each field. To skip the prompts, pass them as flags: `cargo xtask changeset add --help` lists them.

Or create a Markdown file here by hand, named in kebab-case after the change, for example `plugin-install-legacy-spelling-warning.md`:

```markdown
---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-489]
---

`rover plugin install` warns when a plugin version uses a legacy spelling

`rover plugin install supergraph@latest-2` and `@vX.Y.Z` worked silently. They still work, and now print a deprecation warning naming the modern spelling.
```

That becomes this entry in `CHANGELOG.md`:

```markdown
- **`rover plugin install` warns when a plugin version uses a legacy spelling - @dotdat fixes ROVER-489**

  `rover plugin install supergraph@latest-2` and `@vX.Y.Z` worked silently. They still work, and now print a deprecation warning naming the modern spelling.
```

## Format

### Frontmatter

| Field | Required | Value |
| --- | --- | --- |
| `category` | yes | `feat` (🚀 Features), `fix` (🐛 Fixes), or `maint` (🛠 Maintenance) |
| `breaking` | yes | `true` if the change breaks an existing invocation, output format, or configuration. Breaking entries are listed under ❗ BREAKING ❗, whatever their category, and counted in the release's "potentially breaking changes" line. |
| `authors` | no | GitHub handles, with or without `@` |
| `pr` | no | The pull request number |
| `fixes` | no | Issues the change fixes: `ROVER-123`, or `"#123"` for a GitHub issue. Quote a `#` reference, or YAML reads it as a comment. |
| `part_of` | no | Issues the change is part of, in the same forms |

### Body

Write it like a commit message. The first line is the summary, which becomes the entry's bold heading. It can't contain `**`. Anything after it is the entry's detail, indented under the heading.

## Checking

CI runs `mise run check-changeset` on every pull request. It fails when the branch adds no changeset or adds an invalid one. To check locally against a different base branch, run `mise run check-changeset -- --base <branch>`. To validate every changeset in this directory, run `cargo xtask changeset check --all`.

Pull requests that don't need a changeset are exempt:

- pull requests that only change `docs/`
- Renovate and Dependabot pull requests
- any pull request with the `skip-changeset` label, for changes that aren't user-visible

## Releasing

The release pull request runs `cargo xtask changeset release <version>`. It:

1. adds a `# [<version>] - <date>` section to `CHANGELOG.md` with every changeset, grouped by section, newest first
2. writes the same notes to `.changeset/notes/v<version>.md` for the GitHub release
3. deletes the changesets it used

To see what the next release's notes would be, run `cargo xtask changeset preview`.
