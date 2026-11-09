---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-512]
---

`rover dev` reads settings from a `.env` file as it does from the environment

`rover dev` loads a `.env` file in the working directory, but a setting in it was missed when Rover had already read it from the environment: a graph ref in `APOLLO_GRAPH_REF`, which was ignored, and the settings that have a flag, such as `APOLLO_ELV2_LICENSE`, `APOLLO_ROVER_DEV_ROUTER_VERSION` and `APOLLO_LOG_LEVEL`. `rover dev` now loads `.env` before it reads any of them. A variable that is already set in the environment still takes precedence over the same one in `.env`, and other commands still don't read `.env`.
