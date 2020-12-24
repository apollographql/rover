# Release Checklist

This is a list of the things that need to happen during a release.

## Build a Release

### Prepare the Changelog (Full release only)

If you are releasing a beta or a release candidate, no official changelog is needed, but you're not off the hook! You'll need to write testing instructions in lieu of an official changelog.

1. Open the associated milestone. All issues and PRs should be closed. If
   they are not you should reassign all open issues and PRs to future
   milestones.
1. Go through the commit history since the last release. Ensure that all PRs
   that have landed are marked with the milestone. You can use this to
   show all the PRs that are merged on or after YYY-MM-DD:
   `https://github.com/issues?utf8=%E2%9C%93&q=repo%3Aapollographql%2Fapollo-cli+merged%3A%3E%3DYYYY-MM-DD`
1. Go through the closed PRs in the milestone. Each should have a changelog
   label indicating if the change is documentation, feature, fix, or maintenance. If
   there is a missing label, please add one. If it is a breaking change, also add a changelog - BREAKING label.
1. Choose an emoji for the release. Try to make it semi-related to something that's been included in the release.
1. Add this release to the `CHANGELOG.md`. Use the structure of previous
   entries.

### Update cargo manifest

1. Update the version in `Cargo.toml`.
1. Run `cargo update`.
1. Run `cargo test --workspace`.
1. Make sure you have `npm` installed, and run `cargo build`.

### Start a release PR

1. Create a new branch "#.#.#" where "#.#.#" is this release's version (release) or "#.#.#-rc.#" (release candidate)
1. Push up a commit with the `Cargo.toml`, `Cargo.lock`, `CHANGELOG.md`, and `./installers/npm` changes. The commit message can just be "#.#.#" (release) or "#.#.#-rc.#" (release candidate)
1. Request review from the Apollo GraphQL tooling team.

### Review

Most review comments will be about the changelog. Once the PR is finalized and approved:

1. If you made changes, squash or fixup all changes into a single commit. Use the `Squash and Merge` github button.

### Tag and build release

This part of the release process is handled by GitHub Actions, and our binaries are distributed as GitHub Releases. When you push a version tag, it kicks off an action that creates a new GitHub release for that tag, builds release binaries and attaches them to the release.

1. Once ready to merge, checkout `main` branch locally and pull latest changes.
1. Tag the commit by running either `git tag -a v#.#.# -m "#.#.#"` (release), or `git tag -a v#.#.#-rc.# -m "#.#.#-rc.#"` (release candidate)
1. Run `git push --tags`.
1. Wait for CI to pass.

### Edit the release

After CI builds the release binaries and they appear on the [releases page](https://github.com/apollographql/apollo-cli/releases), click `Edit` and update release notes.

#### If this is a stable release (not a release candidate)

1. Paste the current release notes from `CHANGELOG.md` into the release body.
1. Update the *title* of the release (not the tag itself) to include the emoji for the current release
1. Be sure to add any missing link definitions to the release.

#### If this is a release candidate

1. CI should already mark the release as a `pre-release`. Double check that it's listed as a pre-release on the release's `Edit` page.
1. If this is a new rc (rc.0), paste testing instructions into the release notes.
1. If this is a rc.1 or later, the old release candidate testing instructions should be moved to the latest release candidate testing instructions, and replaced with the following message:

   ```markdown
   This beta release is now out of date. If you previously installed this release, you should reinstall and see what's changed in the latest [release](https://github.com/apollographql/apollo-cli/releases).
   ```

   The new release candidate should then include updated testing instructions with a small changelog at the top to get folks who installed the old release candidate up to speed.

## Publish

1. Hit the big green Merge button on the release PR.
1. Check out the tag you pushed with `git checkout v#.#.#`

### Publish to crates.io (full release only)

**IMPORTANT: This step is the hardest to fix if you mess it up. Do not run this step for Release Candidates**.

We don't publish release candidates to crates.io because they don't (as of this writing) have a concept of a "beta" version.

1. Run `cargo test`
1. (Release only) `cargo publish`

### Publish to npm

Full releases are tagged `latest`. Release candidates are tagged `beta`. If for some reason you mix up the commands below, follow the troubleshooting guide.

1. If this is a full release, `cd installers/npm && npm publish`. 
1. If it is a release candidate, `cd installers/npm && npm publish --tag beta`

## Troubleshooting a release

Mistakes happen. Most of these release steps are recoverable if you mess up.

### I pushed the wrong tag

Tags and releases can be removed in GitHub. First, [remove the remote tag](https://stackoverflow.com/questions/5480258/how-to-delete-a-remote-tag):

``` console
git push --delete origin vX.X.X
```

This will turn the release into a `draft` and you can delete it from the edit page.

Make sure you also delete the local tag:

``` console
git tag --delete vX.X.X
```

## I forgot to add the `beta` tag to my RC when I ran `npm publish`

Never fear! We can fix this by updating npm tags. First, add a beta tag for the version you just published:

``` console
npm dist-tag add @apollo/rover@x.x.x-rc.x beta
```

once you add the beta tag, you can list your tags

``` console
npm dist-tag ls @apollo/rover
```

You should now see two tags pointing to the version you just pushed; for example if you had tried to push v0.1.0-rc.0:

``` console
$ npm dist-tag ls @apollo/rover
beta: 0.1.0-rc.0
latest: 0.1.0-rc.0
```

Go back to the Changelog or GitHub releases, find the _actual_ latest version, and re-tag it as latest:

``` console
npm dist-tag add @apollo/rover@x.x.x latest
```

List tags again and you should see the latest restored, and your new release candidate as beta (e.g. 0.1.0-rc.0 is beta and 0.0.0 was last stable version)

``` console
npm dist-tag ls @apollo/rover
beta: 0.1.0-rc.0
latest: 0.0.0
```
