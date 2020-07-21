# Configuration

```
rover-config
🔐  Manage configuraiton

USAGE:
    rover config <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    api-key    🔑 Configure an account or graph API key
    clear      🚮 Remove all configuration
    help       Prints this message or the help of the given subcommand(s)
    profile    💁 Operations for listing, viewing, and deleting configuration profiles
```

The `rover config` command allows you to set and manage configuration.

- `rover config api-key` will allow you to authenticate using an Apollo API key
- `rover config profile` will allow you to manage sets of configuration
- `rover config clear` will allow you to remove all configuration related to the `rover` CLI
