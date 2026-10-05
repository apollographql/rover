---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover supergraph compose` no longer mislabels a failure to create its temporary directory

If Rover couldn't create the temporary directory it writes the resolved supergraph config into, it reported "Failed to run the composition binary" and dropped the underlying cause entirely — describing a step it had not reached, and saying nothing about the one that failed. It now names the real step and shows the cause.
