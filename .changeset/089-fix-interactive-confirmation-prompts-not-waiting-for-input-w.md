---
category: fix
breaking: false
fixes: ["#1455"]
---

Fix interactive confirmation prompts not waiting for input when stdout (but not stdin) is redirected

`y/N` confirmation prompts (the ELv2 license acceptance prompt, `rover graph delete`/`rover subgraph delete` confirmations) checked whether *stdout* was attached to a terminal before reading an answer. Redirecting stdout while leaving stdin interactive made the prompt print, then immediately read an empty answer instead of waiting for input, defaulting to "no" and erroring out. Prompts now read from stdin directly, gated on stdin's own terminal status.
