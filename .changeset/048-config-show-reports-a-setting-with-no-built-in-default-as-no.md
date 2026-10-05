---
category: feat
breaking: false
authors: [dotdat]
---

`config show` reports a setting with no built-in default as `none` (text) or `null` (JSON), not the string `"none"`

`rover config show`'s `value` field is now `null` in `--format json` for a setting that falls all the way through to the builtin tier with no default of its own (currently only `APOLLO_GRAPH_REF`, once its own profile tier lands), rather than the literal string `"none"` - which is itself a valid value for some settings, so a script could no longer tell an unset setting apart from a real one that happened to be set to that string. Text output is unaffected, still rendering `none`.
