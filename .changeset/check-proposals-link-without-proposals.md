---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Schema checks no longer link to a proposal page when no proposals are associated with the changes

When the proposals task of `rover graph check` or `rover subgraph check` found no proposals related to the changes, the output linked to a proposal detail page that didn't correspond to any proposal. It now links to the check itself, or to the variant's list of checks when no link to the check is available, and explains what the configured proposals check severity level means for the result, or that the proposals check didn't run because an earlier task failed. JSON output is unchanged.
