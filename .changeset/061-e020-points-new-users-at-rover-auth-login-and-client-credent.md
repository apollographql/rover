---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-487]
---

E020 points new users at `rover auth login` and client credentials

With no configuration profiles, commands like `rover auth whoami` failed with E020, whose suggestion only mentioned `rover config auth` and `$APOLLO_KEY`. It now leads with `rover auth login` and mentions `$APOLLO_CLIENT_ID`/`$APOLLO_CLIENT_SECRET` for CI; the E020 explanation says the same.
