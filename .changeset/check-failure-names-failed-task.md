---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Schema checks name the task that failed instead of "an unknown check task"

When a `rover subgraph check` or `rover graph check` workflow failed because of a task with no result to show, such as a build that failed without Studio returning its composition errors or a contract filter task, Rover said the change "resulted in an unknown check task to fail". The error now names the failed task.

With `--format json`, `data.task_statuses` lists every task in the check with its status and Studio link, as `{"task", "status", "target_url"}`, so a script can tell which task failed without reading the error message. `task` is one of `build`, `contract_filter`, `operations`, `lint`, `proposals`, `custom` or `downstream`. `json_version` is unchanged.
