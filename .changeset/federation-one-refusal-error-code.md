---
category: fix
breaking: false
authors: [dotdat]
pr: 3984
fixes: [ROVER-493]
---

Refusing Federation 1 has its own error code, E064

A request for Federation 1 (`federation_version: 1`, `latest-0`, `latest-1` or `=0.x`, `rover plugin install supergraph@1`, or a Federation 1 `rover init` template) was refused with a plain `error:` and `error.code: null` in `--format json`. It now carries E064, which `rover explain E064` describes. `rover plugin install` and `rover install --plugin` now refuse it when they run rather than while parsing arguments, so `--format json` reports it in the usual `error` envelope. A bare `rover plugin install` refuses a Federation 1 version declared in the manifest or recorded in the lockfile too, instead of downloading it.
