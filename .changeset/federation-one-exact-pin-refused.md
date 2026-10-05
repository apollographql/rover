---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-498]
---

A `=1.x.y` Federation pin is refused instead of quietly composing on Federation 2

`federation_version: =1.0.0` in `supergraph.yaml` isn't a version the supergraph plugin's parser accepts, so Rover dropped the config's version and carried on with the latest Federation 2 (downloading it, with the opt-in), ignoring the pin. It now fails with the Federation 1 message, as `=0.x`, `1` and `latest-1` already did. `--federation-version =1.0.0` (and `v1.2.3`, `=0.10.0`) fail with the same message, and the error for a value that isn't a supported version no longer offers `1`, which Rover refuses.
