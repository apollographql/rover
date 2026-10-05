---
category: feat
breaking: false
authors: [ebylund]
---

Add `--preserve-comments` to `rover persisted-queries generate`

Keeps the contiguous comment block directly above each operation in the generated operation body, instead of dropping it during normalization. Comments elsewhere in a document are still dropped. Off by default; manifests generated without the flag are byte-for-byte unchanged. Because an operation's ID is a hash of its body, enabling this changes the ID of every operation that has a preceding comment, so every client generating a manifest for the same graph must enable it too.
