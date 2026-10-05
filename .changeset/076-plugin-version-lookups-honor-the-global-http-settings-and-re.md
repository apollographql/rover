---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Plugin version lookups honor the global HTTP settings and retry transient failures

Resolving a floating plugin version (`latest-2`, `latest`) — the lookup `rover supergraph compose`, `rover dev`, `rover connector`, `rover lsp`, and `rover install --plugin` make before downloading — used its own HTTP client. It ignored `--client-timeout` and `--insecure-accept-invalid-certs`/`--insecure-accept-invalid-hostnames`, had no timeout at all (a hung registry connection could stall a run indefinitely), and failed on the first transient error. It now uses Rover's configured client, bounds each attempt at 10 seconds, and retries connection failures, timeouts, and 5xx/408/425/429 responses for up to 10 seconds (or `--client-timeout`, if shorter) before falling back to an already-installed plugin or failing.
