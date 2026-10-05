---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

Plugin install failures report their own error codes

When Rover can't obtain a plugin — for `rover install --plugin`, or on the fly for `rover supergraph compose`, `rover dev`, `rover connector`, and `rover lsp` — the error now carries a stable code under `error.code`, so a script can tell the failures apart: `E048` when the requested version can't be resolved against the plugin registry, `E049` when the artifact can't be downloaded, and `E050` when it can't be unpacked or written into the install directory. Each comes with a suggestion naming the plugin and what to do next, in place of the previous generic ones. Run `rover explain <CODE>` for details.

An exact version the registry has no artifact for is told apart from a failed download: a release the registry says was withdrawn (`410 Gone`) is reported as `E051`, naming where the version was set — `rover install --plugin`, a version flag, an environment variable, or `supergraph.yaml` — and the newest release in the same major to set there instead, and a version the registry never published (`404 Not Found`) is reported as `E048`.
