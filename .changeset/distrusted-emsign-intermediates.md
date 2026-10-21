---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Stop trusting distrusted emSign intermediate certificates

Rover's TLS stack could accept certificate chains through emSign intermediates that are no longer trusted, and bundled root certificates that trusted them (RUSTSEC-2026-0382, RUSTSEC-2026-0383). Rover now uses `rustls-webpki` 0.103.16 and `webpki-root-certs` 1.0.10, which reject them.
