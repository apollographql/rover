---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-486]
---

`rover auth logout` and E024 suggest a `rover config delete` command that works

Both told you to run `rover config delete --profile <NAME>`, but `config delete` takes the profile name as a positional argument, so following the suggestion failed with a missing-argument error. They now suggest `rover config delete <NAME>`.
