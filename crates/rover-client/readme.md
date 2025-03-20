# Rover Client

This is the client used by rover to make network requests. This README covers aspects of the client that are useful to know when developing and using it.

The rover client uses [Reqwest](https://docs.rs/reqwest/latest/reqwest/) and some familiarity with that crate is useful in both developing and using it.

# Development :: WIP

We're in the midst of undergoing a transition from a synchronous, blocking client used by threads to an asynchronous one used by an event loop (a tokio runtime that also uses threads, but is non-blocking). Because of that, some of the naming and ergonomics might feel weird.

# Using the client

## Timeouts

By default, the timeout is 10s. This is set _not_ by the `MAX_ELAPSED_TIME` const in the `rover-client/src/blocking/client.rs` file, but in the `Default` implementation for `ClientTimeout` in `rover/src/utils/client.rs`. Users can pass the flag `--client-timeout` with an integer representing seconds to control the overall client timeout.

## Retries

### Overview
Retries can happen for two broad reasons: either the client failed or the server failed. Retries are also enabled by default, but can be disabled by passing an argument to the client's `execute` method called `should_retry` (a boolean).

Retries are also only part of the story. The interval of time you place between retries matters. If you retry all at once, you might get rate-limited or otherwise fail. It's best to spread them out exponentially, adding big chunks of time so that the server can complete its work and get ready for more work. Spreading out retries is a good idea, but if you have multiple calls happening at the same time with the same spread, they might fail if the server is overloaded. It's better to spread them out with some added noise, meaning that you spread them out with some randomly generated bit of time added or subtracted so that calls are received by the server in a somewhat distributed fashion.

#### `backon` crate

We use the [backon](https://crates.io/crates/backon) crate for retries with exponential backoff and jitter.

Retries are limited based on the total duration of the retries. The `MAX_ELAPSED_TIME` in the client file sets this value and defaults to 10s.

#### Client failures

Retries happen when either the client times out (there's a flag for setting the timeout, but by default it's 10s), when there's a connection error, or when incomplete messages are received. Errors about the request or response body, decoding, building the client, or redirecting the network call aren't retried.

#### Server failures

Retries happen for general server errors (notably, _all_ statuses between 500-99),  but not when the request is ill-formed as identified by the server (that is, a 400).






