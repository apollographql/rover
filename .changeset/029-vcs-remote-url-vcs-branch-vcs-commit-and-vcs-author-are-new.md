---
category: feat
breaking: false
authors: [dotdat]
---

`--vcs-remote-url`, `--vcs-branch`, `--vcs-commit`, and `--vcs-author` are new global flags, paired with the existing `APOLLO_VCS_*` environment variables

These override the Git context (remote URL, branch, commit, author) reported to GraphOS on check/publish. Previously env-var-only; the flag wins when both are set, and omitting both still falls back to the value inferred from the current directory's Git repository exactly as before.
