---
category: feat
breaking: false
authors: [SharkBaitDLS]
part_of: [ROVER-448]
---

`rover plugin install` reports each plugin and a summary when installing several

Installing more than one plugin prints each plugin's outcome as it completes, including each failure in full, and ends with a summary such as `3 plugins: 2 installed, 1 already installed.` When any plugin failed, the summary is the command's error and carries the first failure's code. With `--format json`, `data.plugins` lists what installed, `data.failed` lists every failure as its own error object, and the top-level `error` is the first failure.
