---
category: feat
breaking: true
authors: [dotdat]
---

Remove Federation 1 support

Rover no longer supports Federation 1. `rover supergraph compose`, `rover dev`, `rover lsp`, `rover connector`, and `rover install --plugin supergraph@<version>` now reject any Federation 1 version (for example `federation_version: 1` in `supergraph.yaml`, or `latest-0`/an exact `0.x` version for the plugin installer) with an error pointing at the [Federation 2 migration guide](https://www.apollographql.com/docs/federation/federation-2/moving-to-federation-2#opt-in-to-federation-2). `rover init` no longer maps a template's federation version to a Federation 1 build pipeline track.
