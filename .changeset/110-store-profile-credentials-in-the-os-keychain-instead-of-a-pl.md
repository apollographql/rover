---
category: maint
breaking: false
authors: [dotdat]
---

Store profile credentials in the OS keychain instead of a plaintext file

`rover config auth` now stores each profile's API key in the OS-native credential store (Keychain on macOS, Credential Manager on Windows, the kernel keyring on Linux), falling back to a permission-hardened (`0600`/`0700`) JSON file when no native keychain is available — for example, headless Linux/CI, or an unsigned local build on macOS. Existing plaintext `$APOLLO_CONFIG_HOME/profiles/<profile>/.sensitive` files are transparently migrated the first time they're read, then removed. `rover config auth`, `whoami`, `list`, `delete`, and `clear` all behave the same as before, and the `APOLLO_KEY` environment variable override is unaffected. On some platforms the OS may now prompt for keychain access the first time a credential is read or written in a session. If Rover ever fails to read, write, or delete a credential, it now surfaces a dedicated error, E046, instead of a generic failure.
