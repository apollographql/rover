# Homebrew Formula Update for Shell Completions

This document describes the changes needed to the Homebrew formula to automatically install bash and zsh completion scripts when Rover is installed via Homebrew.

## Formula Location

The Rover Homebrew formula is maintained at:
https://github.com/Homebrew/homebrew-core/blob/master/Formula/r/rover.rb

## Required Changes

Update the `install` method in the formula to generate and install completion scripts:

```ruby
def install
  # Ensure that the `openssl` crate picks up the intended library.
  ENV["OPENSSL_DIR"] = Formula["openssl@3"].opt_prefix
  ENV["OPENSSL_NO_VENDOR"] = "1"

  system "cargo", "install", *std_cargo_args

  # Generate and install bash completion
  bash_completion.install Utils.safe_popen_read(bin/"rover", "completion", "bash")

  # Generate and install zsh completion
  zsh_completion.install Utils.safe_popen_read(bin/"rover", "completion", "zsh")
end
```

## Explanation

- `bash_completion.install` installs the bash completion script to `share/bash-completion/completions/rover`
- `zsh_completion.install` installs the zsh completion script to `share/zsh/site-functions/_rover`
- `Utils.safe_popen_read` executes the `rover completion` command and captures its output
- The completion scripts are generated dynamically using the `rover completion` subcommand

## Testing

After updating the formula:

1. Install Rover via Homebrew: `brew install rover`
2. Verify bash completion is installed: `ls $(brew --prefix)/share/bash-completion/completions/rover`
3. Verify zsh completion is installed: `ls $(brew --prefix)/share/zsh/site-functions/_rover`
4. Test completion in your shell (may require restarting the shell or sourcing completion files)

## Notes

- Bash completion requires `bash-completion` to be installed (usually via `brew install bash-completion@2`)
- Zsh completion should work automatically if zsh is configured properly
- Users may need to restart their shell or source completion files for completions to take effect

