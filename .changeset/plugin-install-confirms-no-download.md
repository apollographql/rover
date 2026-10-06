---
category: fix
breaking: false
authors: [dotdat]
---

`rover plugin install` confirms an install that downloads nothing

A plugin that was already installed (a re-run, `--no-download`, or a floating version satisfied by what was on disk) made `rover plugin install` exit successfully without printing anything, so it looked as though nothing had happened. It now says on stderr which plugin and version it found and where, for example "the 'supergraph' plugin v2.9.3 is already installed at …". A download still reports itself as before, and `--format json` is unchanged.
