# Changelog

All notable changes to Rover will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# [0.0.2] - 2021-02-23

## 🚀 Features

- **Better logging experience - [EverlastingBugstopper], [pull/263]**

  When passing `--log debug`, the logs are now pretty printed with their call location.

  Additionally, progress messages are no longer printed with an `INFO` prefix on every line,
  messages are displayed to the user with no mess and no fuss.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/263]: https://github.com/apollographql/rover/pull/263

- **Add useful info to debug logs - [EverlastingBugstopper], [pull/268]**

  When running Rover with `--log debug`, you can now see which environment variables are being used
  and the raw JSON payload returned by the Apollo Studio API.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/268]: https://github.com/apollographql/rover/pull/268

- **Provide a better error message for malformed API Keys - [EverlastingBugstopper], [issue/215] [pull/275]**

  Before, if you passed a malformed API key, the error message was "406: Not Acceptable", since that's
  what the Apollo Studio API returned. Rover now provides you with a
  much more actionable error message.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/275]: https://github.com/apollographql/rover/pull/275
  [issue/215]: https://github.com/apollographql/rover/issues/215

- **Add support for M1 Macbooks - [EverlastingBugstopper], [issue/295] [pull/297]/[pull/300]**

  Big Sur allows the new M1 Macbooks to run code compiled for the `x86_64` architecture in emulation
  mode, even though the machines themselves have an `arm64` architecture. We have updated
  our `curl | sh` installer and our `npm` installer to reflect this, and anybody running Big Sur
  on the new M1 machines can now install and use Rover.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/297]: https://github.com/apollographql/rover/pull/297
  [pull/300]: https://github.com/apollographql/rover/pull/300
  [issue/295]: https://github.com/apollographql/rover/issues/295

- **Add a `> ` prompt to the `rover config auth` command - [EverlastingBugstopper], [issue/279] [pull/281]**

  It was a bit confusing to be presented with a blank line after running `rover config auth`.
  To make it more clear that this is a prompt for an API key, we now print `> ` at the beginning
  of the prompt.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/281]: https://github.com/apollographql/rover/pull/281
  [issue/279]: https://github.com/apollographql/rover/issues/279

## 🐛 Fixes

- **Allow Rover to be used outside the context of a git repository - [JakeDawkins], [issue/271] [pull/282]**

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/282]: https://github.com/apollographql/rover/pull/282
  [issue/271]: https://github.com/apollographql/rover/issues/271

- **Always use the shorthand ref when generating Git Context - [lrlna], [pull/255]**

  Rover now computes the shorthand ref and specifies that as the "branch", even
  if the specific ref is not necessarily a branch (such as a tag).

  [lrlna]: https://github.com/lrlna
  [pull/255]: https://github.com/apollographql/rover/pull/255

- **Do not send telemetry events for dev builds - [EverlastingBugstopper], [pull/258]**

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/258]: https://github.com/apollographql/rover/pull/258

- **Fix a typo in the README - [abernix], [pull/269]**

  `s/Rover co[ma]{3}nds to interact/Rover commands to interact/`

  [abernix]: https://github.com/abernix
  [pull/269]: https://github.com/apollographql/rover/pull/269

## 🛠 Maintenance

- **Address latest style suggestions (clippy) - [lrlna], [pull/267]**

  A new version of [clippy](https://rust-lang.github.io/rust-clippy/rust-1.50.0/index.html)
  gave us some pointers for more idiomatic code style, and we addressed them!

  [lrlna]: https://github.com/lrlna
  [pull/267]: https://github.com/apollographql/rover/pull/267

- **Unify terminal text color dependency - [EverlastingBugstopper], [pull/276]**

  Now we only use `ansi_term` for providing colored text.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/276]: https://github.com/apollographql/rover/pull/276

## 📚 Documentation

- **Document Git Context - [JakeDawkins], [pull/262]**

  We added documentation for how Rover provides Git Context to Apollo Studio.
  You can read all about it [here](https://apollo-cli-docs.netlify.app/docs/rover/configuring/#git-context).

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/262]: https://github.com/apollographql/rover/pull/262

- **Fix npx usage documentation - [abernix], [issue/Issue #] [pull/270]**

  We updated the docs to show that it is necessary to pass 
  `--package @apollo/rover` each time Rover is invoked through `npx`.

  [abernix]: https://github.com/abernix
  [pull/270]: https://github.com/apollographql/rover/pull/270
  [issue/Issue #]: https://github.com/apollographql/rover/issues/Issue #

- **Update layout of Rover's intro article - [StephenBarlow], [issue/Issue #] [pull/259]**

  The intro article in Rover's docs were reordered to put the info about the Public preview 
  towards the bottom of the page, so more relevant information is no longer below the fold.

  [StephenBarlow]: https://github.com/StephenBarlow
  [pull/259]: https://github.com/apollographql/rover/pull/259
# [0.0.1] - 2021-02-09

**Initial beta release.** Please visit [our documentation page](https://apollographql.com/docs/rover/) for information on usage.
