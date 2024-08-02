# Tests

This README is a work in progress. If anything is confusing or unclear, please open an issue or pull request!

## End-to-end tests

Our end-to-end tests can be run either locally or in GitHub. We use an example supergraph from Apollo Solutions called [retail supergraph](https://github.com/apollosolutions/retail-supergraph) as a basis for many of the tests.

### Developing new end-to-end tests

#### Windows and Unix

We're developing for _both_ unix-based systems (Linux, MacOS) and Windows. This leads to some surprising things (like how environment variables are handled; you can't just prepend `SOME_VAR=` in powershell), which can be checked for by running the smoke tests (detailed below). 

#### `RetailSupergraph`

A struct with methods for interacting with the retail supergraph (e.g., getting subgraph names or URLs) and related metadata (e.g., the working directory where the retail supergraph was git cloned).  

#### Fixtures

Fixtures allow us to reuse either data or functionality across tests. We use [rstest](https://docs.rs/rstest/latest/rstest)'s [fixtures](https://docs.rs/rstest/latest/rstest/attr.fixture.html) and being familiar with that crate will be useful when developing tests for Rover.

Some tests need subgraphs to be running (e.g., `rover dev`), but others don't. To use running subgraphs, use the `run_subgraphs_retail_supergraph` fixture. This fixture returns `RetailSupergraph` and the methods from that struct can be used in tests. See the `e2e/dev.rs` for an example.

For those that only need certain data about the supergraph config, you can use the fixture `retail_supergraph`. See `e2e/supergraph/compose.rs` for an example.


#### Fixtureless tests

Some tests (like testing subcommand or option failures) don't need to use any fixture at all. That's okay! Don't feel like you need to add fixtures when they're irrelevant to your test. And, if a fixture's use doesn't make sense, raise a ticket so that we can either make it clearer in code or the documentation.


### Locally

#### Gotchas

Some tests require Apollo-only access. These generally access resources that are gated behind having the right level of privilege (e.g., supergraphs, graphOS organizations, and so on). If you attempt to run all the tests at once and see failures, this might be why. 

For Apollo folks wanting to develop tests locally, ask someone from the Rover team or browse the internal Rover docs for information on how to run Rover with the right level of privileges.


#### Pre-requisites

You'll need to be able to use `cargo` and have some familiarity with `cargo test`, but you'll also need to install NodeJS and Git because these are used to clone and then build and run a set of subgraphs that rover commands are issued against.

#### Running the tests

The following command runs our end-to-end tests, which are `ignored` intentionally because we don't want to run them along with all of our tests because that would slow down our local feedback loop tremendously:

`cargo test e2e -- --ignored`

Note: use `--nocapture` to get print statements:

`cargo test e2e -- --ignored --nocapture`

Particular tests can be targeted for a faster feedback loop:

`cargo test e2e::supergraph::compose::it_fails_without_a_config -- --ignored --nocapture`

#### ERRADDRINUSE

Sometimes you'll see an error for the target address (localhost with some port) being in use. You can safely ignore these locally. When we first start the retail subgraphs, the same port gets used. Nodemon eventually gets things up and running and it's nothing about your code that's producing those errors (unless you're adding something new that uses ports and you've selected one already in use).

### GitHub

#### Manual smoke tests

To run against our supported architectures and operating systems, go to the [manual smoke tests in GitHub Actions](https://github.com/apollographql/rover/actions/workflows/run-smokes-manual.yml), select `run workflow`, select your branch, and then use JSON arrays to specify the federation versions and router versions. Here's an example for a single federation version and a single router version:

```
["2.8.0"]
["1.50.0"]
```

These would be input individually, which will make sense when you see the input prompt.


#### Automatic smoke tests

TBD :: we'll likely run the smoke tests against each commit in the future, but this isn't the current state of the world. So, use manual smoke tests for now.

