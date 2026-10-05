---
category: maint
breaking: false
authors: [dotdat]
---

Simplify the `rover-print` API to stop propagating write errors

`Print`/`PrintExt` methods (`print`, `print_line`, `infoln`, `warnln`, `errln`, `successln`) no longer return `std::io::Result<()>`. A failed terminal write now records a best-effort `tracing::error!` diagnostic (visible when running with `--log`) instead of being propagated; most callers previously discarded the error with `let _ = ...`, so this is a no-op for them. A few call sites (`rover config auth`, `rover config whoami`, `rover persisted-queries generate`) previously used `?` and would fail the whole command on a print error — those now continue and succeed instead, which is an intentional, if obscure, behavior change.
