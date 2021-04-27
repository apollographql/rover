# Changelog

All notable changes to Rover will be documented in this file.

This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!-- # [x.x.x] - 2021-mm-dd
> Important: X breaking changes below, indicated by **❗ BREAKING ❗**
## 🚀 Features
## ❗ BREAKING ❗
## 🐛 Fixes
## 🛠 Maintenance
## 📚 Documentation --> 

# [0.0.10] - 2021-04-27

## 🚀 Features

- **Prints information about opting out of anonymized usage data collection after installation - [EverlastingBugstopper], [pull/456]**

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/456]: https://github.com/apollographql/rover/pull/456

- **Report SHA-256 hash of git remote URL - [EverlastingBugstopper], [issue/313] [pull/461]**

  Our anonymized usage data will now report the hash of a git remote URL if it exists to more accurately determine the number of unique projects Rover is used in.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/461]: https://github.com/apollographql/rover/pull/461
  [issue/313]: https://github.com/apollographql/rover/issues/313

- **Client returns an error on non-200 status codes - [EverlastingBugstopper], [pull/472]**

  Sometimes when performing HTTP requests, a non-200 status code is returned. This is now properly handled, and Rover's HTTP client will return a proper error message informing you of the bad response.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/472]: https://github.com/apollographql/rover/pull/472

## 🐛 Fixes

- **Adds a newline to all output to stdout - [EverlastingBugstopper], [issue/458] [pull/462]**

  Rover no longer has the bug where it wouldn't conclude its output with a newline. Now we don't make your shells upset! 

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/462]: https://github.com/apollographql/rover/pull/462
  [issue/458]: https://github.com/apollographql/rover/issues/458

## 🛠 Maintenance

- **Automatically add triage labels to issues created with templates - [JakeDawkins], [pull/448]**

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/448]: https://github.com/apollographql/rover/pull/448

- **Refactor API key loading - [EverlastingBugstopper], [pull/463]**

  Made a slight performance refactor to how we load our API keys that reduces the number of filesystem reads.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/463]: https://github.com/apollographql/rover/pull/463

- **Update dependency crates - [EverlastingBugstopper], [pull/470]**

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/470]: https://github.com/apollographql/rover/pull/470

## 📚 Documentation

- **Updates language of migration guide - [StephenBarlow], [pull/446]**

  [StephenBarlow]: https://github.com/StephenBarlow
  [pull/446]: https://github.com/apollographql/rover/pull/446


# [0.0.9] - 2021-04-13

> This release is a small release to fix the automated release process, and should be considered an extension of the previous (v0.0.8) release

## 🛠 Maintenance

- ** Fix boolean logic in release action - [EverlastingBugstopper], [pull/442]**

  0.0.8 was released to npm as a beta by accident because of an environment variable being treated as a boolean when it's a string. This just fixes that for a new release.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/442]: https://github.com/apollographql/rover/pull/442

# [0.0.8] - 2021-04-13

## 🚀 Features

- **Users can now install Rover with a shorter URL - [JakeDawkins], [issue/287] [pull/426]**

  Instead of downloading Rover's install script from the tagged GitHub URL, you can now use the much simpler endpoints:

  https://rover.apollo.dev/install/nix/latest and https://rover.apollo.dev/install/windows/latest.

  You can see our [documentation](https://www.apollographql.com/docs/rover/getting-started/) for more info on the new installation pattern.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/426]: https://github.com/apollographql/rover/pull/426
  [issue/287]: https://github.com/apollographql/rover/issues/287

- **Print link to documentation after installation - [EverlastingBugstopper], [issue/141] [pull/437]**

  After a user installs Rover, we now print a link to the getting started page at
  https://go.apollo.dev/r/docs.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/437]: https://github.com/apollographql/rover/pull/437
  [issue/141]: https://github.com/apollographql/rover/issues/141

## 🐛 Fixes

- **Deserialize supergraph configuration to a deterministic data structure - [lrlna], [issue/422] [pull/423]**

  Previously, when Rover deserialized a supergraph configuration file, it did so using a HashMap.
  This made the results of `rover supergraph compose` non-deterministic, which was undesirable. 
  By switching from a HashMap to a BTreeMap, `rover supergraph compose` is now deterministic.

  [lrlna]: https://github.com/lrlna
  [pull/423]: https://github.com/apollographql/rover/pull/423
  [issue/422]: https://github.com/apollographql/rover/issues/422

## 🛠 Maintenance

- **Update telemetry URL - [JakeDawkins], [pull/427]**

  Telemetry is now routed through a Netlify function instead of a Cloudflare Worker.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/427]: https://github.com/apollographql/rover/pull/427


## 📚 Documentation

- **Add Apollo CLI to Rover migration guide - [JakeDawkins], [issue/284] [pull/425]**

  Rover now has a migration guide for users coming from the Apollo CLI. You can see it [here](https://www.apollographql.com/docs/rover/migration).

  [jakedawkins]: https://github.com/JakeDawkins
  [pull/425]: https://github.com/apollographql/rover/pull/425
  [issue/284]: https://github.com/apollographql/rover/issues/284

# [0.0.7] - 2021-04-01
## 🐛 Fixes
- **Updates URL base in Core Schema output to specs.apollo.dev - [abernix], [pull/418]**

  [abernix]: https://github.com/abernix
  [pull/418]: https://github.com/apollographql/rover/pull/418

## 📚 Documentation
- **Added specificity to Rover's public preview period docs - [ndintenfass], [pull/415]**

  [ndintenfass]: https://github.com/ndintenfass
  [pull/415]: https://github.com/apollographql/rover/pull/415

- **Small categorization update - [StephenBarlow], [pull/414]**

  [StephenBarlow]: https://github.com/StephenBarlow
  [pull/414]: https://github.com/apollographql/rover/pull/414

# [0.0.6] - 2021-03-31
## 🚀 Features
- **Add postinstall instructions - [EverlastingBugstopper], [pull/406]**

  Adds a message after an install on how to add Rover to a user's PATH.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/406]: https://github.com/apollographql/rover/pull/406

## 🐛 Fixes
- **Change 'CSDL' to 'Core Schema' - [lrlna], [pull/403]**

  Output of `rover supergraph compose` is a Core Schema. Our output
  previously indicated that it was a `CSDL`.

  [lrlna]: https://github.com/lrlna
  [pull/403]: https://github.com/apollographql/rover/pull/403

- **Remove Rover binary before overwriting it - [EverlastingBugstopper], [issue/398] [pull/400]**

  Updating Rover with `curl` required a restart on MacOS due to Apple's
  Gatekeeper not recognizing that a new package has been installed on a
  user's system. This forces the installer to remove previously Rover
  directory and its installation and do a fresh installation.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [issue/398]: https://github.com/apollographql/rover/issues/398
  [pull/400]: https://github.com/apollographql/rover/pull/400

- **Adds entitlements to MacOS signed binaries - [EverlastingBugstopper], [issue/399] [pull/405]**

  `rover supergraph compose` process has been getting `killed` on MacOS in
  `v0.0.5` release. This was happening due to the fact that we are using
  `deno-core` to execute composition, a package that requires access to
  memory management. This fix adds an Entitelement when notarizing Rover that
  specifically allows for unsigned packages to have access to memory
  management.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [issue/399]: https://github.com/apollographql/rover/issues/399
  [pull/405]: https://github.com/apollographql/rover/pull/405


# [0.0.5] - 2021-03-30

> Important: 2 breaking changes below, indicated by **❗ BREAKING ❗**

## 🚀 Features
- **Adds introspection ability for subgraphs - [lrlna], [issue/349] [pull/377]**

  A new command, `rover subgraph introspect` has been added. This command
  runs a _federated introspection_ query against a server which has
  implemented the requirements of the [federation
  specification](https://www.apollographql.com/docs/federation/federation-spec/).
  This command accepts endpoint headers (`-H`, `--header`) for making the introspection
  request (if required) and outputs SDL to stdout.

  [lrlna]: https://github.com/lrlna
  [pull/377]: https://github.com/apollographql/rover/pull/377
  [issue/349]: https://github.com/apollographql/rover/issues/349

- **Fallback to monochromic output in installer when `tput` is unavailable - [abernix], [issue/371] [pull/372]**

  The `tput` command allows easier ANSI output using named values in rather than
  control characters.

  While we could just use control characters and maintain colored output in the
  absence of `tput`, it's probably also reasonable to gracefully fall back to
  monochromatic output.

  [abernix]: https://github.com/abernix
  [pull/372]: https://github.com/apollographql/rover/pull/372
  [issue/371]: https://github.com/apollographql/rover/issues/371

## ❗ BREAKING ❗
- **Renames `core build` to `supergraph compose` - [lrlna], [pull/391]**

  To align with other Apollo teams on the usage of `supergraph` and
  `composition`, we are renaming `core build` to `supergraph compose`.

  [lrlna]: https://github.com/lrlna
  [pull/391]: https://github.com/apollographql/rover/pull/391

- **Updates harmonizer@0.2.2 - [abernix], [pull/396]**

  Updates harmonizer to the latest version. This version now composes and
  returns a core schema instead of CSDL. CSDL was an internal implementation
  of composition and this new format is meant to bring some stability to `rover
  supergraph compose`.

  [abernix]: https://github.com/abernix
  [pull/396]: https://github.com/apollographql/rover/pull/396

## 🐛 Fixes
- **Handle 400-599 HTTP responses - [lrlna], [issue/394] [issue/187] [pull/395]**

  Previously, Rover did not provide errors for any HTTP requests that return a status code between 400 and 599. This fix makes sure Rover checks for those errors before moving on to parsing the response body.

  This fix also does an extra check for 400 errors, as the Apollo
  Server sends additional information that we can display to users.

  [lrlna]: https://github.com/lrlna
  [issue/394]: https://github.com/apollographql/rover/issues/394
  [issue/187]: https://github.com/apollographql/rover/issues/187
  [pull/395]: https://github.com/apollographql/rover/pull/395

## 🛠 Maintenance
- **Sign and notarize MacOS binaries as part of CI - [EverlastingBugstopper], [pull/363]**

  This automates our signing and notarization process when releasing MacOS
  binaries. This is especially necessary to install and run Rover on latest
  M1s and Big Sur.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/363]: https://github.com/apollographql/rover/pull/363

- **Test, build and release on ubuntu-16.04 - [abernix], [pull/381]**

  This pins us to Ubuntu 16.04 which ships with glib 2.19. This should allow
  us to work with a wider range of operating systems than the newer glib
  that we get with Ubuntu 20.04, which is ubuntu-latest on GitHub Actions
  Virtual Environments (which resulted in a Rover that wouldn't run on
  Ubuntu 18.04).

  Ubuntu 16.04 is LTS until April 2024, and is still receiving active
  updates through the LTS program.

  [abernix]: https://github.com/abernix
  [pull/381]: https://github.com/apollographql/rover/pull/381

- **Cache Rust artifacts in CI linter job- [EverlastingBugstopper], [pull/365]**

  The rest of our GitHub actions workflows pull from the cache to take
  advantage of Rust's incremental compilation. We now do this for clippy
  too so it finishes (and fails) faster.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/365]: https://github.com/apollographql/rover/pull/365

- **Addresses new clippy 1.51 warning - [EverlastingBugstopper], [pull/364]**

  Addresses some stylistic problems noticed by the new version of our linter [clippy](https://github.com/rust-lang/rust-clippy/blob/master/CHANGELOG.md#rust-151)

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/364]: https://github.com/apollographql/rover/pull/364

## 📚 Documentation
- **Update documentation for 0.0.5 release- [JakeDawkins] [StephenBarlow] [EverlastingBugstopper], [pull/389]**

  Documents recent additions to Rover in detail, including `rover supergraph
  compose`, `rover subgraph introspect` and `rover graph introspect`.

  [JakeDawkins]: https://github.com/JakeDawkins
  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [StephenBarlow]: https://github.com/StephenBarlow
  [pull/389]: https://github.com/apollographql/rover/pull/389

# [0.0.4] - 2021-03-23

> Important: Two breaking changes below, indicated by **❗ BREAKING ❗**

## 🚀 Features

- **Core schema building capabilities - [EverlastingBugstopper], [pull/340]**

  Adds a new command, `rover core build` to handle building 
  [core schema documents](https://apollo-specs.github.io/core/draft/pre-0/)
  from multiple subgraph schemas. This also adds a new config format to support
  this command in YAML. Currently, this is only documented in [pull/340].

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/340]: https://github.com/apollographql/rover/pull/340

- **Adds introspection ability for graphs - [lrlna], [issue/180] [issue/171] [pull/283]**

  A new command, `rover graph introspect` has been added, usable for introspecting
  graphs (not subgraphs). This command accepts endpoint [headers] for making the
  introspection request (if required) and outputs SDL to stdout.

  [lrlna]: https://github.com/lrlna
  [pull/283]: https://github.com/apollographql/rover/pull/283
  [issue/180]: https://github.com/apollographql/rover/issues/180
  [issue/171]: https://github.com/apollographql/rover/issues/171
  [headers]: https://github.com/apollographql/rover/pull/351

## ❗ BREAKING ❗

- **Rename `push` to `publish` everywhere - [JakeDawkins], [issue/344] [pull/347]**

  "Publish" is a more correct name for what these commands do. We wanted to be
  extra clear about its name matching its functionality, so any usage of `push`
  commands should now be `publish`.

  [JakeDawkins]: https://github.com/JakeDawkins
  [pull/347]: https://github.com/apollographql/rover/pull/347
  [issue/344]: https://github.com/apollographql/rover/issues/344

- **Rename `committer` to `author` - [EverlastingBugstopper], [issue/338] [pull/339]**

  Note: This is only  breaking change if you were previously using the `APOLLO_VCS_COMMITTER` 
  env variable. If so, migrate by changing that environment variable to `APOLLO_VCS_AUTHOR`.

  Changes the underlying git context implementation to report the `author` of a commit rather than a `committer`. This is primarily intended to properly link the real author with a commit in Apollo Studio.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/339]: https://github.com/apollographql/rover/pull/339
  [issue/338]: https://github.com/apollographql/rover/issues/338

## 🐛 Fixes

- **Output composition errors to stderr - [EverlastingBugstopper], [pull/335]**

  There was an incorrect usage of `tracing::error`, causing composition errors
  to not show up when expected. This change unifies this error printing with the
  rest of the project.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/335]: https://github.com/apollographql/rover/pull/335

## 🛠 Maintenance

- **Auto-bump versions in install scripts - [EverlastingBugstopper], [pull/332]**

  Added auto version bumping in the build script, so there's no chance the Rover
  team will miss this important step in the release process.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/332]: https://github.com/apollographql/rover/pull/332

- **Don't print by default for automatic update checks - [EverlastingBugstopper], [pull/342]**

  When Rover automatically checks for updates every 24 hours, it no longer will
  print if there is no update available. It will still print for manual checks
  and if Rover is out of date.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/342]: https://github.com/apollographql/rover/pull/342

- **Add metadata to `Cargo.toml` - [EverlastingBugstopper], [pull/346]**

  In preparation for future releases to [crates.io](https://crates.io), we've
  added relevant metadata to the Cargo.toml file for the project.

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/346]: https://github.com/apollographql/rover/pull/346

## 📚 Documentation

- **Adds `APOLLO_HOME` variable to docs - [EverlastingBugstopper], [pull/341]**

  `APOLLO_HOME` is the override variable for Rover's parent directory, where the
  binary lives. This was previously undocumented, but now it's not!

  [EverlastingBugstopper]: https://github.com/EverlastingBugstopper
  [pull/341]: https://github.com/apollographql/rover/pull/341



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

- **Verify paths are all valid utf-8 - [EverlastingBugstopper], [pull/326]**

  Just to make our code more safe and easier to maintain, we now check and make
  sure paths are all valid utf-8 to make sure any non utf-8 paths won't cause unexpected issues.

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

- **Output Service title for graph keys in whoami command - [lrlna], [issue/280] [pull/299]**

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
