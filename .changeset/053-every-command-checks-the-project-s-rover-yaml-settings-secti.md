---
category: feat
breaking: false
authors: [dotdat]
---

Every command checks the project's `rover.yaml` `settings:` section before it runs

Run inside a project, every command first reads the `settings:` section of the `rover.yaml` that plugin discovery finds. A credential in that section fails the command before it sends any request, including the update check (`E060`), as does one setting spelled both ways (`E061`). In these cases Rover prints one warning and the command carries on:

- an unrecognized key
- a `settings:` section in the user-level `rover.yaml`
- a project manifest Rover can't read or parse No setting's value is applied from the file yet. Outside a project, nothing changes.
