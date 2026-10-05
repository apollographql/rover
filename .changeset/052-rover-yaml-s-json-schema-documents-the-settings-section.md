---
category: feat
breaking: false
authors: [dotdat]
---

`rover.yaml`'s JSON schema documents the `settings:` section

The project manifest's `settings:` section is read separately from `plugins:`, so a problem in one section never refuses the other. That holds even for a repeated key under `plugins:`. Once project settings are wired up, these cases print a warning and apply nothing, instead of failing the command:

- a `settings:` section in the user-level `rover.yaml`, whose warning points at `rover config set`
- a project manifest Rover can't read or parse at all
- a top-level YAML merge key (`<<`)
