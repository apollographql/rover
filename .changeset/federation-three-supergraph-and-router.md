---
category: feat
breaking: false
authors: [dotdat]
pr: 4004
---

Federation 3 `supergraph` and `router` versions

`rover plugin install supergraph@=3.0.0-preview.1`, `supergraph@3` and `supergraph@latest-3` used to be refused ("Must be 'latest-2' or an exact version preceded with an '='"), and `federation_version: =3.0.0-preview.1` in `supergraph.yaml` was silently dropped. Rover now accepts a Federation 3 version everywhere a Federation version can be written: `rover plugin install`, `supergraph.yaml`, `--federation-version`, and the plugin manifest. It installs and composes with it, under the same ELv2 license as Federation 2. `router@3` and `router@=3.x.y` are accepted too, and so is the older `latest-3` spelling for both plugins (on the command line and in the manifest), with the usual deprecation warning asking for `3`. `latest` still means the newest 2.x. Resolving a floating `3` or `latest-3` asks the plugin registry for its newest 3.x, which the registry has to support; installing an exact `=3.x.y` does not. This needs `apollo-federation-types` 0.18 and `apollo-language-server` 0.11, which Rover now depends on.
