---
category: feat
breaking: false
authors: [dotdat]
---

Composition errors hint at Federation 1 subgraphs that need migrating

When composition fails, no federation version was specified, and none of the subgraphs uses the `@link` directive, Rover now adds a suggestion that the subgraphs may have been written for Federation 1, which Rover no longer supports, with a link to the migration guide. Rover can't tell a Federation 1 subgraph from a Federation 2 subgraph that uses no federation directives, so the suggestion only appears after composition has failed, and only says the subgraphs may need migrating. The errors themselves are unchanged.
