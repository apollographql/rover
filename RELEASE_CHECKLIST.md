# Release Checklist

This is a list of the things that need to happen during a release.

## Build a Release

### Prepare the Changelog (Full release only)

If you are releasing a beta or a release candidate, no official changelog is needed, but you're not off the hook! You'll need to write testing instructions in lieu of an official changelog.

1. Open the associated [milestone](https://github.com/apollographql/rover/milestones), it should be called `vNext`.
1. Rename the milestone to the version you are currently releasing and set the date to today
1. Create a new empty `vNext` milestone
1. If there are any open issues/PRs in the milestone for the current release, move them to the new `vNext` milestone.
1. Go through the commit history since the last release. Ensure that all PRs
   that have landed are marked with the milestone. You can use this to
   show all the PRs that are merged on or after YYYY-MM-DD:
   `https://github.com/issues?utf8=%E2%9C%93&q=repo%3Aapollographql%2Frover+merged%3A%3E%3DYYYY-MM-DD`
1. Go through the closed PRs in the milestone. Each should have a changelog
   label indicating if the change is documentation, feature, fix, or maintenance. If
   there is a missing label, please add one. If it is a breaking change, also add a BREAKING label.
1. Add this release to the `CHANGELOG.md`. Use the structure of previous
   entries. An example entry looks like this:

> - **Fixes Input Value Definition block string encoding for descriptions.  - @lrlna, #1116 fixes #1088**
>
>   Input values are now multilined when a description is present to allow for a more readable generated SDL.

As you can see, there is a brief description, followed by the author's GitHub handle, the PR number and the issue number. If there is no issue associated with a PR, just put the PR number.

### Start a release PR

1. Make sure you have both `npm`, `cargo` and `cargo install graphql_client_cli` installed on your machine and in your `PATH`.
1. Create a new branch "#.#.#" where "#.#.#" is this release's version (release) or "#.#.#-rc.#" (release candidate)
1. Update the version in [`./Cargo.toml`](./Cargo.toml), workspace crates like `rover-client` should remain untouched.
1. Update the installer versions in [`docs/source/getting-started.mdx`](./docs/source/getting-started.mdx) and [`docs/source/ci-cd.mdx`](./docs/source/ci-cd.mdx). (eventually this should be automated).
1. Run `cargo run -- help` and copy the output to the "Command-line Options" section in [`README.md`](./README.md#command-line-options).
1. Run `cargo xtask prep` (this will require `npm` to be installed).
1. Push up all of your local changes. The commit message should be "release: v#.#.#" or "release: v#.#.#-rc.#"
1. Open a Pull Request from the branch you pushed.
1. Add the release pull request to the milestone you opened.
1. Paste the changelog entry into the description of the Pull Request.
1. Add the "🚢release" label to the PR.

### Review

Most review comments will be about the changelog. Once the PR is finalized and approved:

1. If you made changes, squash or fixup all changes into a single commit. Use the `Squash and Merge` github button.

### Tag and build release

This part of the release process is handled by CircleCI, and our binaries are distributed as GitHub Releases. When you push a version tag, it kicks off a workflow that checks out the tag, builds release binaries for multiple platforms, and creates a new GitHub release for that tag.

1. Wait for tests to pass.
1. Have your PR merged to `main`.
1. Once merged, run `git checkout main` and `git pull`.
1. Sync your local tags with the remote tags by running `git tag -d $(git tag) && git fetch --tags`
1. Tag the commit by running either `git tag -a v#.#.# -m "#.#.#"` (release), or `git tag -a v#.#.#-rc.# -m "#.#.#-rc.#"` (release candidate)
1. Run `git push --tags`.
1. Wait for CI to pass.
1. Watch the release show up on the [releases page](https://github.com/apollographql/rover/releases)
1. Click `Edit`, paste the release notes from the changelog, and save the changes to the release.

#### If this is a stable release (not a release candidate)

1. Paste the current release notes from `CHANGELOG.md` into the release body.

#### If this is a release candidate

1. CI should already mark the release as a `pre-release`. Double check that it's listed as a pre-release on the release's `Edit` page.
1. If this is a new rc (rc.0), paste testing instructions into the release notes.
1. If this is a rc.1 or later, the old release candidate testing instructions should be moved to the latest release candidate testing instructions, and replaced with the following message:

   ```markdown
   This beta release is now out of date. If you previously installed this release, you should reinstall and see what's changed in the latest [release](https://github.com/apollographql/rover/releases).
   ```

   The new release candidate should then include updated testing instructions with a small changelog at the top to get folks who installed the old release candidate up to speed.

## Troubleshooting a release

Mistakes happen. Most of these release steps are recoverable if you mess up.

## The release build failed after I pushed a tag!

That's OK! In this scenario, do the following. 

1. Try re-running the job, see if it fixes itself
1. If it doesn't, try re-running it with SSH and poke around, see if you can identify the issue
1. Delete the tag either in the GitHub UI or by running `git push --delete origin vX.X.X`
1. Make a PR to fix the issue in [`.circleci/config.yml`](./.circleci/config.yml)
1. Merge the PR
1. Go back to the "Tag and build release" section and re-tag the release. If it fails again, that's OK, you can keep trying until it succeeds.

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

### The release worked but the installer is not working.

In this case, you should yank the version so npm packages and packages downloading from `rover.apollo.dev/{platform}/latest` are not affected.

   - Follow the same steps as above to delete the release and the tag.
   - Run `npm unpublish @apollo/rover@vX.X.X`

## I forgot to add the `beta` tag to my RC when I ran `npm publish`

Never fear! We can fix this by updating npm tags. First, add a beta tag for the version you just published:

```console
npm dist-tag add @apollo/rover@x.x.x-rc.x beta
```

once you add the beta tag, you can list your tags

```console
npm dist-tag ls @apollo/rover
```

You should now see two tags pointing to the version you just pushed; for example if you had tried to push v0.1.0-rc.0:

```console
$ npm dist-tag ls @apollo/rover
beta: 0.1.0-rc.0
latest: 0.1.0-rc.0
```

Go back to the Changelog or GitHub releases, find the _actual_ latest version, and re-tag it as latest:

```console
npm dist-tag add @apollo/rover@x.x.x latest
```

List tags again and you should see the latest restored, and your new release candidate as beta (e.g. 0.1.0-rc.0 is beta and 0.0.0 was last stable version)

```console
npm dist-tag ls @apollo/rover
beta: 0.1.0-rc.0
latest: 0.0.0
```
