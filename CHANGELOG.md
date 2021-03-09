# Changelog

All notable changes to Rover will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

# [0.0.3] - 2021-03-09

## 🚀 Features

- **❗ BREAKING ❗ Squash `config show` functionality into `config whoami` - [EverlastingBugstopper], [issue/274] [pull/323]**

  Since the only thing that `rover config show` did was show the saved api key,
  it made sense to squash that functionality into the `whoami` command. We decided
  that we'd prefer not to ever expose the full api key to stdout (you can still
  find it in the saved config file), but we still show the first and last 4 
  characters of it to help with debugging.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/323]: https://github.com/apollographql/rover/pull/323
  [issue/274]: https://github.com/apollographql/rover/issues/274

- **Add api key origin to `whoami` command - [EverlastingBugstopper], [issue/273] [pull/307]**

  The `whoami` command, which is used to verify api keys and help with debugging now
  shows where that key came from, either a `--profile` or the `APOLLO_KEY` env variable.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/307]: https://github.com/apollographql/rover/pull/307
  [issue/273]: https://github.com/apollographql/rover/issues/273

- **`rover docs` commands to make viewing documentation easier - [EverlastingBugstopper], [issue/308] [pull/314]**

  To make it easier to find and navigate Rover's docs, we added two commands: 
  `rover docs list` to list helpful docs pages and `rover docs open` to open a
  docs page in the browser.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/314]: https://github.com/apollographql/rover/pull/314
  [issue/308]: https://github.com/apollographql/rover/issues/308

- **Better errors and suggestions for invalid variants - [EverlastingBugstopper], [issue/208] [pull/316]**

  Previously, Rover would tell you if you tried accessing an invalid variant, 
  but couldn't provide any recommendations. This adds recommendations for simple
  typos, lists available variants for graphs with small numbers of variants, and
  provides a link to view variants in Apollo Studio for graphs with many variants.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/316]: https://github.com/apollographql/rover/pull/316
  [issue/208]: https://github.com/apollographql/rover/issues/208

- **Remove the need to reload terminal after install - [EverlastingBugstopper], [issue/212] [pull/318]**
  
  Rather than asking users to reload their terminal after install, we do the
  extra work of sourcing Rover's env file after install, preventing linux users
  from having to do that or reload the terminal themselves.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/318]: https://github.com/apollographql/rover/pull/318
  [issue/212]: https://github.com/apollographql/rover/issues/212

- **Rover automatically checks for updates - [JakeDawkins], [issue/296] [pull/319]**

  Every 24 hours, Rover will automatically check for new releases and let you know.
  You can also run the `rover update check` command to manually check for updates.
  If an update is available, Rover warns once per day at most and provides a link
  to the docs for update instructions.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/319]: https://github.com/apollographql/rover/pull/319
  [issue/296]: https://github.com/apollographql/rover/issues/296

- **Update installers to be consistent and not require version variables - [JakeDawkins], [issue/88] [pull/324]**

  This provides a consistent experience when installing Rover. When running the
  linux install script, you no longer are required to pass a `VERSION`, but still
  may if you want to download an older version. The windows installer now supports
  the same `$Env:VERSION` environment variable for similar overrides. By default,
  installer scripts will download the version of rover released with that version
  of the script.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/324]: https://github.com/apollographql/rover/pull/324
  [issue/88]: https://github.com/apollographql/rover/issues/88

- **Verify paths are all valid utf8 - [EverlastingBugstopper], [pull/326]**

  Just to make our code more safe and easier to maintain, we now check and make
  sure paths are all valid utf8 to make sure any non-utf8 paths won't cause unexpected issues.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/326]: https://github.com/apollographql/rover/pull/326

## 🐛 Fixes

- **Consistently refer to configuration instead of config in error messages - [EverlastingBugstopper], [pull/306]**

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/306]: https://github.com/apollographql/rover/pull/306

## 🛠 Maintenance

- **Move all build-time checks for env variables to util - [EverlastingBugstopper], [pull/310]**

  Having a bunch of `env!` macros across the codebase is just less beautiful and
  maintainable than having them in one utility file. This PR just moves all of those
  calls, looking up `CARGO_ENV_*` environment variables to a single place.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/310]: https://github.com/apollographql/rover/pull/310

- **Make output tables prettier - [EverlastingBugstopper], [pull/315]**

  Replaces the characters in table borders with characters that show fewer &
  smaller gaps to make tables look a little more polished :)

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/315]: https://github.com/apollographql/rover/pull/315

- **Add test to make sure install scripts never change names/paths - [EverlastingBugstopper], [pull/321]**

  This adds a simple test to make sure we don't move or rename install scripts
  on accident in the future, since that would be a major breaking change.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/321]: https://github.com/apollographql/rover/pull/321

## 📚 Documentation

- **Instructions for using Rover in CircleCI and GitHub Actions - [JakeDawkins], [issue/245] [pull/329]**

  Some CI providers require a couple of additional steps to get Rover installed
  and working. These docs help get Rover working with linux setups in GitHub
  Actions and CircleCI.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/329]: https://github.com/apollographql/rover/pull/329
  [issue/245]: https://github.com/apollographql/rover/issues/245


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

- **Add a friendlier message for the case of no config profiles - [JakeDawkins], [issue/202] [pull/303]**

  The new user experience, where there are no config profiles found, was a little cryptic.
  To make it easier to understand what the problem is, we added a friendly error message
  asking the user to run `rover config auth`.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/303]: https://github.com/apollographql/rover/pull/303
  [issue/202]: https://github.com/apollographql/rover/issues/202

- ** Output Service title for graph keys in whoami command - [lrlna], [issue/280] [pull/299]**

  `rover config whoami` was displaying `Name` information which was unclear
  in the context of this command. Instead of `Name`, we are now displaying
  `Service title` information for graph keys, and omitting `Name` and
  `Service Title` for user keys, as the already existing information provides
  enough information for `User`.

  [lrlna]: https://github.com/lrlna
  [pull/299]: https://github.com/apollographql/rover/pull/299
  [issue/280]: https://github.com/apollographql/rover/issues/280

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

- **Hide API key printing in debug logs - [JakeDawkins], [pull/302]**

  We no longer print a user's api key in the `--log debug` logs when
  saving a key (from `rover config auth`)/

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/302]: https://github.com/apollographql/rover/pull/302

## 📚 Documentation

- **Document Git Context - [JakeDawkins], [pull/262]**

  We added documentation for how Rover provides Git Context to Apollo Studio.
  You can read all about it [here](https://apollo-cli-docs.netlify.app/docs/rover/configuring/#git-context).

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/262]: https://github.com/apollographql/rover/pull/262

- **Fix npx usage documentation - [abernix], [pull/270]**

  We updated the docs to show that it is necessary to pass 
  `--package @apollo/rover` each time Rover is invoked through `npx`.

  [abernix]: https://github.com/abernix
  [pull/270]: https://github.com/apollographql/rover/pull/270

- **Update layout of Rover's intro article - [StephenBarlow], [pull/259]**

  The intro article in Rover's docs were reordered to put the info about the Public preview
  towards the bottom of the page, so more relevant information is no longer below the fold.

  [StephenBarlow]: https://github.com/StephenBarlow
  [pull/259]: https://github.com/apollographql/rover/pull/259
# [0.0.1] - 2021-02-09

**Initial beta release.** Please visit [our documentation page](https://apollographql.com/docs/rover/) for information on usage.
