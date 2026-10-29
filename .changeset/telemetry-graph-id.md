---
category: feat
breaking: false
authors: [dotdat]
part_of: [ROVER-424]
---

Anonymous usage telemetry includes an identifier for the graph a command ran against

When a command has a graph ref, Rover's anonymous usage data now includes a `graph_id` derived from it, so usage can be grouped by graph without revealing the graph. It is a keyed hash: the key is a random value Rover generates on first use and stores in `install_salt.txt` beside `machine.txt`, and never sends. Nobody can reproduce the identifier by hashing known graph refs, and it groups invocations from one install only, because other installs have different keys. Deleting `install_salt.txt` resets it. Commands without a graph ref send no `graph_id`, and nothing is generated or stored when telemetry is disabled.
