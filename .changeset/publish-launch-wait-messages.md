---
category: fix
breaking: false
authors: [dotdat]
fixes: [ROVER-496]
---

Publish launch-wait messages read correctly, and the timeout has an error code, E065

When `rover graph publish` or `rover subgraph publish` stopped waiting for its launch, the error said "Timed out waiting for the launch to complete, or raise APOLLO_CHECKS_TIMEOUT_SECONDS.", a sentence missing its clause, and carried no code. It now says "Timed out waiting for the launch to complete.", with the suggestion to raise the timeout beneath it, and carries E065, which `rover explain E065` describes. One failed downstream contract launch is now reported as "a downstream contract launch failed" instead of "downstream contract launch failed".
