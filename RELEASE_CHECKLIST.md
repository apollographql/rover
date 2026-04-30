# Release Checklist

This is a list of the things that need to happen during a release. We support three kinds of releases:
1. A standard release, from `main` (i.e. v0.27.0, v0.28.3, v1.5.2, etc.)
2. A release candidate release, from `main` (i.e. v0.27.0-rc.0, v1.5.3-rc.3, etc.)
3. A pre-release release, from any branch (i.e. v0.26.0-PR1234, v0.27.4-my.build.0)

_N.B All the version numbers above follow Semantic Versioning, and specifically its notion of encoding pre-release information after the `-` for more information see the [Semantic Versioning specification](https://semver.org/)_

## Standard Release

As noted above, a standard release takes the form of a tagged commit on the `main` branch, and includes all
commits previous to it. Preparing it involves several phases:

### Prepare The Changelog

1. Open the associated [milestone](https://github.com/apollographql/rover/milestones), it should be called `vNext`.
2. Rename the milestone to the version you are currently releasing and set the date to today
3. Create a new empty `vNext` milestone
4. If there are any open issues/PRs in the milestone for the current release, move them to the new `vNext` milestone.
5. Go through the commit history since the last release. Ensure that all merged PRs, are marked with the milestone.  The searches below may be helpful when performing this task
   1. [This search](https://github.com/apollographql/rover/pulls?q=is%3Apr+is%3Aclosed+is%3Amerged+merged%3A%3E%3D2024-09-01) lists PRs merged before a specific date, update this to the day after the last release to make sure nothing is missed
   2. [This search](https://github.com/apollographql/rover/pulls?q=is%3Apr+is%3Aclosed+is%3Amerged+merged%3A%3E%3D2024-09-01+no%3Amilestone) lists PRs merged before a specific date that do not have a milestone attached.
   3.    **Note:** If a PR is merged and then reverted before the release there's no need to include both PRs in the CHANGELOG.
6. Go through the closed PRs in the milestone. Each should have a changelog
   label indicating if the change is documentation, feature, fix, or maintenance. If
   there is a missing label, please add one. If it is a breaking change, also add a BREAKING label.
7. Add this release to the `CHANGELOG.md`. Use the structure of previous
   entries. An example entry looks like this:

   > - **Fixes Input Value Definition block string encoding for descriptions - @lrlna, #1116 fixes #1088**
   >
   >   Input values are now multilined when a description is present to allow for a more readable generated SDL.

   As you can see, there is a brief description, followed by the author's GitHub handle, the PR number and the issue number. If there is no issue associated with a PR, just put the PR number.

### Create a release PR

1. Make sure you have both `npm`, `cargo` and `graphql_client_cli` installed on your machine and in your `PATH`.
2. Create a new branch "#.#.#" where "#.#.#" is this release's version
3. Update the version in [`./Cargo.toml`](./Cargo.toml), workspace crates like `rover-client` should remain untouched.
4. Update the installer versions in [`docs/source/getting-started.mdx`](./docs/source/getting-started.mdx) and [`docs/source/ci-cd.mdx`](./docs/source/ci-cd.mdx). (eventually this should be automated).
5. Run `cargo run -- help` and copy the output to the "Command-line Options" section in [`README.md`](./README.md#command-line-options).
6. Run `cargo xtask prep` (this will require `npm` to be installed).
7. Push up all of your local changes. The commit message should be "release: v#.#.#"
8. Open a Pull Request from the branch you pushed.
9. Add the release pull request to the milestone you opened.
10. Paste the changelog entry into the description of the Pull Request.
11. Add the "🚢release" label to the PR.
12. Get the PR reviewed
    1. If this necessitates making changes, squash or fixup all changes into a single commit. Use the `Squash and Merge` GitHub button.

### Tag and build release

This part of the release process is handled by GitHub Actions, and our binaries are distributed both as GitHub Releases and as Docker images to ghcr.io and Dockerhub. When you push a version tag, it kicks off a workflow that checks out the tag, builds release binaries and images for multiple platforms, and creates a new GitHub release for that tag.

1. Have your PR merged to `main`.
2. Once merged, run `git checkout main` and `git pull`.
3. Sync your local tags with the remote tags by running `git tag -d $(git tag) && git fetch --tags`
4. Tag the commit by running either `git tag -a v#.#.# -m "#.#.#"`
5. Run `git push --tags`.
6. Wait for CI to pass.
7. Watch the release show up on the [releases page](https://github.com/apollographql/rover/releases)
8. Click `Edit`, paste the release notes from the changelog, and save the changes to the release.
9. Close the milestone for this release.

### Verify The Release

1. Run `npm dist-tag ls @apollo/rover` and check the version listed next to latest is the expected one
2. Head to the [Rover Documentation](https://www.apollographql.com/docs/rover/getting-started/) and install the latest version on your machine
3. Run some commands against that version to ensure the binary runs
4. Install Rover using Dockerhub (`docker pull apollograph/rover`)
5. Run some commands against it to ensure the container works (`docker run apollograph/rover:latest <<args>>`)

## Release Candidate Builds

These are releases that usually proceed a standard release as a way of getting features to customers faster

### Create a release PR

1. Make sure you have both `npm`, `cargo` and `graphql_client_cli` installed on your machine and in your `PATH`.
2. Create a new branch "#.#.#-rc.#" where "#.#.#" is this release's version, and the final `#` is the number of the release candidate (this starts at 0 and increments by 1 for each subsequent release candidate)
3. Update the version in [`./Cargo.toml`](./Cargo.toml), workspace crates like `rover-client` should remain untouched.
4. Run `cargo run -- help` and copy the output to the "Command-line Options" section in [`README.md`](./README.md#command-line-options).
5. Run `cargo xtask prep` (this will require `npm` to be installed).
6. Push up all of your local changes. The commit message should be "release: v#.#.#-rc.#"
7. Open a Pull Request from the branch you pushed. The description for this PR should include the salient changes in this release candidate, and what testing should be applied to it.
8. Paste the changelog entry into the description of the Pull Request.
9. Add the "🚢release" label to the PR.
10. Get the PR reviewed
    1. If this necessitates making changes, squash or fixup all changes into a single commit. Use the `Squash and Merge` GitHub button.

### Tag and build release

This part of the release process is handled by GitHub Actions, and our binaries are distributed both as GitHub Releases and as Docker images to ghcr.io and Dockerhub. When you push a version tag, it kicks off a workflow that checks out the tag, builds release binaries and images for multiple platforms, and creates a new GitHub release for that tag.

1. Have your PR merged to `main`.
2. Once merged, run `git checkout main` and `git pull`.
3. Sync your local tags with the remote tags by running `git tag -d $(git tag) && git fetch --tags`
4. Tag the commit by running either `git tag -a v#.#.#-rc.# -m "#.#.#-rc.#"`
5. Run `git push --tags`.
6. Wait for CI to pass.
7. Watch the release show up on the [releases page](https://github.com/apollographql/rover/releases)
8. CI should already mark the release as a `pre-release`. Double check that it's listed as a pre-release on the release's `Edit` page.
9. If this is a new rc (rc.0), paste testing instructions into the release notes.
10. If this is a rc.1 or later, the old release candidate testing instructions should be moved to the latest release candidate testing instructions, and replaced with the following message:

    ```markdown
    This beta release is now out of date. If you previously installed this release, you should reinstall and see what's changed in the latest [release](https://github.com/apollographql/rover/releases).
    ```

    The new release candidate should then include updated testing instructions with a small changelog at the top to get folks who installed the old release candidate up to speed.

### Verify The Release

1. Run `npm dist-tag ls @apollo/rover` and check the version listed next to beta is the expected one, and that `latest` matches that which is marked as `latest` in GitHub.
2. Head to the [Rover Documentation](https://www.apollographql.com/docs/rover/getting-started/) and install the latest version on your machine
3. Run some commands against that version to ensure the binary runs
4. Install Rover using Dockerhub (`docker pull apollograph/rover:#.#.#-rc.#`)
5. Run some commands against it to ensure the container works (`docker run apollograph/rover:#.#.#-rc.# <<args>>`)

## Pre-Release Release

Sometimes it's necessary to create a `rover` release from an arbitrary branch, this mostly happens in the situation where we want to quickly iterate on a customer fix.

### Create a release branch

1. Make sure you have both `npm`, `cargo` and `graphql_client_cli` installed on your machine and in your `PATH`.
2. Create a new branch `#.#.#-<<IDENTIFIER>>` where "#.#.#" is this release's version.
   1. `IDENTIFIER` can be any series of valid [Semver identifiers separated by dots](https://semver.org/#spec-item-9)
3. Update the version in [`./Cargo.toml`](./Cargo.toml), workspace crates like `rover-client` should remain untouched.
4. Run `cargo xtask prep` (this will require `npm` to be installed).
5. Push up all of your local changes.

### Tag and build release

This part of the release process is handled by GitHub Actions, and our binaries are distributed both as GitHub Releases and as Docker images to ghcr.io and Dockerhub. When you push a version tag, it kicks off a workflow that checks out the tag, builds release binaries and images for multiple platforms, and creates a new GitHub release for that tag.

1. Once merged, run `git checkout <<YOUR_BRANCH>>` and `git pull`.
2. Sync your local tags with the remote tags by running `git tag -d $(git tag) && git fetch --tags`
3. Tag the commit by running either `git tag -a v#.#.#-<<IDENTIFIER>> -m "#.#.#-<<IDENTIFIER>>"`
4. Run `git push --tags`.
5. Wait for CI to pass.
6. Watch the release show up on the [releases page](https://github.com/apollographql/rover/releases)
7. CI should already mark the release as a `pre-release`. Double check that it's listed as a pre-release on the release's `Edit` page.

### Verify The Release

1. Run `npm dist-tag ls @apollo/rover` and check the version listed next to beta is the expected one, and that `latest` matches that which is marked as `latest` in GitHub.
2. Head to the [Rover Documentation](https://www.apollographql.com/docs/rover/getting-started/) and install the latest version on your machine
3. Run some commands against that version to ensure the binary runs
4. Install Rover using Dockerhub (`docker pull apollograph/rover:#.#.#-<<IDENTIFIER>>`)
5. Run some commands against it to ensure the container works (`docker run apollograph/rover:#.#.#-<<IDENTIFIER>> <<args>>`)

### Post-Release Cleanup

1. If you intend to publish more builds you can leave your branch as-is, however if you have finished, ensure the branch is deleted.

## Troubleshooting a Release

Mistakes happen. Most of these release steps are recoverable if you mess up.

### The release build failed after I pushed a tag!

That's OK! In this scenario, do the following. 

1. Try re-running the job, see if it fixes itself
2. If it doesn't, try re-running it with SSH and poke around, see if you can identify the issue
3. Delete the tag either in the GitHub UI or by running `git push --delete origin vX.X.X`
4. Make a PR to fix the issue in [`.github/workflows/release.yml`](./.github/workflows/release.yml)
5. Merge the PR
6. Go back to the "Tag and build release" section and re-tag the release. If it fails again, that's OK, you can keep trying until it succeeds.

### I pushed the wrong tag

Tags and releases can be removed in GitHub. First, remove the remote tag:

```console
git push --delete origin vX.X.X
```

This will turn the release into a `draft` and you can delete it from the edit page.

Make sure you also delete the local tag:

```console
git tag --delete vX.X.X
```

#### The wrong tag was published to NPM and/or Dockerhub

Both registries treat release artifacts as effectively immutable, but the
recovery steps differ.

**NPM**

1. Authenticate to your personal npm account; you must be listed as a publisher
   on `@apollo/rover`:
   ```console
   npm login
   ```
2. If the bad publish is **less than 72 hours old**, unpublish it:
   ```console
   npm unpublish @apollo/rover@X.X.X
   ```
3. After 72 hours npm refuses unpublish. Deprecate instead so installers warn:
   ```console
   npm deprecate @apollo/rover@X.X.X "<<reason>>"
   ```

**Dockerhub**

1. Delete the offending tag from the Dockerhub UI (requires admin access on the apollograph org).

**After the wrong tag is removed**

1. Cut a new release at a higher version number. Both Dockerhub and NPM treat verisons as immutable.

### The release worked but the installer is not working.

In this case you want to stop new installs of the broken version, in particular
via npm and `rover.apollo.dev/{platform}/latest`.

1. Follow the process outlined in [I pushed the wrong tag](#i-pushed-the-wrong-tag) to pull the release
   from GitHub, NPM, and Dockerhub.
2. Ship the fix at a **new version number** by pushing a new tag.

### I tried to do a pre-release, but it ended up becoming an actual release

In this case you need to do two things

1. Go to the releases page on GitHub, find your release:
   1. Uncheck `Set as the latest release`
   2. Check `Set as a pre-release`
2. This will restore `latest` to point to the previous latest release (which should be a stable version)
3. Note down the version that `latest` is point to in GitHub
4. Fix the settings in `npm` by running the following command 
    ```console
   npm dist-tag add @apollo/rover@<<VERSION_NUMBER_FROM_STEP_3>> latest
   ```
5. Run the following command to verify your changes:
   ```console
   npm dist-tag list @apollo/rover
   ```
6. It should respond as follows:
   ```console 
   beta: <<PRE_RELEASE_VERSION>>
   latest: <<VERSION_NUMBER_FROM_STEP_3>>
   ```
7. Repoint the Dockerhub `:latest` tag back to the previous stable release.
   You'll need `docker login` against an account with push access to the `apollograph` org:
   ```console
   docker buildx imagetools create -t apollograph/rover:latest apollograph/rover:<<VERSION_NUMBER_FROM_STEP_3>>
   ```
8. Verify `:latest` now points at the correct version and still carries both
   `linux/amd64` and `linux/arm64`:
   ```console
   docker buildx imagetools inspect apollograph/rover:latest
   ```
