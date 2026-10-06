---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-500]
---

`rover auth login --no-open` stops saying it's opening your browser, and `rover auth logout` keeps your settings

- `--no-open` still printed "Opening your browser to authenticate. If it doesn't open automatically, visit this URL: …". With it, Rover now just prints "Visit this URL to authenticate: …".
- `rover auth logout` deleted the whole profile, including settings stored with `rover config set`. It now removes only the credential, as the profile-configuration spec requires, and a profile left with no settings is still removed entirely.
