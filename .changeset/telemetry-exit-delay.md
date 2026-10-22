---
category: fix
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry delays exit by at most 250 milliseconds

Rover now reports its anonymous usage data after a command finishes and waits at most 250 milliseconds for the request, however slow or unreachable the telemetry endpoint is. Previously it could wait up to a second. Command output and exit codes are unchanged.
