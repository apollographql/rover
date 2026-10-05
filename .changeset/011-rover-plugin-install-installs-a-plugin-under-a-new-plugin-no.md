---
category: feat
breaking: false
authors: [SharkBaitDLS]
---

`rover plugin install` installs a plugin, under a new `plugin` noun

`rover plugin install <name>@<version>` does exactly what `rover install --plugin <name>@<version>` does, including `--force` and `--elv2-license`. `rover install --plugin` keeps working, but is now deprecated and prints a warning naming the `rover plugin install` command to use instead. With `--format json`, both report the plugin they installed under `data.plugins`, in the same shape `rover supergraph compose` uses. Plugin install errors now suggest `rover plugin install` too.
