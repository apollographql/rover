---
category: feat
breaking: false
authors: [dotdat]
---

`--profile` is now a global flag, accepted by every command

Previously `--profile` was only defined on commands that talk to GraphOS; commands with no use for a credential (`rover template`, `rover docs`, `rover completion`, `rover update`, `rover install`, `rover info`, `rover explain`) rejected it as an unrecognized argument. It's now resolved once, globally, and every command accepts it — commands that don't use a credential simply ignore it. Omitting `--profile` still resolves to the `default` profile exactly as before; this is the first step of ROVER-451's configuration-precedence work and does not change behavior for anyone who doesn't pass `--profile` today.
