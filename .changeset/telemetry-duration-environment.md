---
category: feat
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry includes command duration and invocation environment

Rover's anonymous usage data now includes how long the command ran, in milliseconds, including writing its output, and whether the invocation looks `automated` (CI was detected, or standard input isn't a terminal) or `interactive`. Neither identifies a machine or a person.
