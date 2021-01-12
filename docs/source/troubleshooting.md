---
title: "Troubleshooting Rover"
sidebar_title: "Troubleshooting"
---

Rover aims to be as user-friendly as possible, especially when things go wrong. If you encounter an issue, see below for help debugging and resolving it. 

## Log levels

Rover's logs are configurable. If a Rover command's default log level provides insufficient information, try setting its `--log` flag to `debug` or `trace` to see as much information as possible.

For details, see [Logging](./configuring#logging).

## Authentication errors

See [Authenticating with Apollo Studio](./configuring#authenticating-with-apollo-studio).

## Error reference

Below is a list of commonly encountered errors in Rover and their most common causes. Some errors have been truncated to fit. 

#### `encountered a GraphQL error, registry responded with: ...`
  - The GraphQL operation seems to be invalid. Check the operation.

#### `invalid header name` or `invalid header value`
  - When building request headers, there was an error with a header name. It might
    not comply with header name rules.
  - If you're using a header name or value that includes spaces, make sure to quote the 
    `"key:value"` pair.

#### `could not parse JSON`
  - Occurs when a GraphQL request incorrectly responds with something besides standard JSON.
  - Confirm that you're sending your request to the correct URL.

#### `encountered an error handling the response: ...`
  - A generic error that can occur in many places. If there are no helpful messages associated with the error, please [open an issue](https://github.com/apollographql/rover/issues).

#### `encountered an error while sending a request`
  - An error propagated from another library. If the message is unhelpful, please [open an issue](https://github.com/apollographql/rover/issues).

#### `The response from the server was malformed...`
  - Both `body.errors` and `body.data` are empty in the server's response. In a valid GraphQL response, at least one of `body.errors` or `body.data` is not empty.
  - If you see this error, check your GraphQL server. It might not be complying
    with the GraphQL spec.

#### `No graph found. Either the graph@variant combination wasn't found...`
  - Confirm that your graph ID and variant name are _both_ valid. This error occurs if _either_ value is invalid.
  - If you confirm both and still encounter this error, view your graph in [Apollo Studio](https://studio.apollographql.com) and check the graph ID in the URL bar. **The graph ID might not match the graph's title.**

#### `The graph X is a non-federated graph...`
  - Usually occurs if you run the `rover subgraph` command on a non-federated graph. Run `rover graph` instead.
  - In other cases, your graph might not be set up yet. Make sure you've pushed at least one subgraph to Apollo before trying again.

#### `Invalid ChangeSeverity.`
  - Occurs if Apollo Studio returns an invalid value during checks.
  - If you encounter this error, please [open an issue](https://github.com/apollographql/rover/issues).

## Feedback

If you are still having issues or you have suggestions for improving Rover, please
[let us know](https://github.com/apollographql/rover/issues)!
