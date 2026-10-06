---
category: fix
breaking: false
authors: [dotdat]
pr: 3993
fixes: [ROVER-502]
---

`rover dev` fails on an invalid `--composition-version`

`rover dev --composition-version =2.15.2` (or `2`) printed an error about the doubled `=` and then carried on with a different composition version. An invalid `--composition-version`, or `APOLLO_ROVER_DEV_COMPOSITION_VERSION`, now fails the command before it starts, with a message that says to use an exact version such as `2.9.0`, with no leading `=`.
