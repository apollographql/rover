---
title: "Troubleshooting"
sidebar_title: "Troubleshooting"
description: "How to debug and troubleshoot issues with Rover"
---

Rover aims to be as user-friendly as possible, especially when things go wrong.
If you're having issues with Rover, this guide will help you debug and get to
the bottom of the issue. 

## Authentication Errors

If you're running into authentication errors with Apollo Studio, check out the
[authenticating with Apollo Studio](./configuring#authenticating-with-apollo-studio)
section of the configuration docs.

## Logging

Rover's logs are configurable. If you're running into an issue where the
available logs are unhelpful, try expanding the log level to `debug` or `trace`
to see the most information possible. To see more about how to configure
logging, see [here](./configuring#logging).

## Error Reference

Below is a list of the most commonly encountered errors in Rover and their
common causes. Some errors have been truncated to fit. 

- `encountered a GraphQL error, registry responded with: ...`
  - The GraphQL operation seems to be invalid. Check the operation.
- `invalid header name` or `invalid header value`
  - When building request headers, there was an error with a header name. It may
    not comply with header name rules.
  - If using a header name or value with spaces, make sure to quote the 
    `"key:value"` pair
- `could not parse JSON`
  - This error happens when a GraphQL request incorrectly responds with
    something that isn't standard JSON.
  - Check the url you're making a request to, and make sure it's correct.
- `encountered an error handling the response: ...`
  - This is a generic error that can happen in many places. If there are no
    helpful messages associated with this error, please [open an issue](https://github.com/apollographql/rover/issues).
- `encountered an error while sending a request`
  - This is an error propagated from another library. If the message is
    unhelpful, please [open an issue](https://github.com/apollographql/rover/issues).
- `The response from the server was malformed...`
  - This error occurs when there are no `body.errors` but `body.data` is
    also empty. In proper GraphQL responses, there should _always_ be either
    body.errors or body.data.
  - If you see this error, check your GraphQL server. It may not be complying
    with the GraphQL spec.
- `No graph found. Either the graph@variant combination wasn't found...`
  - Check to make sure your graph id and variant name are _both_ valid. If
    either of these values are invalid, you'll see this error.
  - If you've already checked these, and are still seeing this error, go to the
    graph in [Apollo Studio](https://studio.apollographql.com) and check the
    graph id in the URL bar. **The graph ID may not match the graph title**
- `The graph X is a non-federated graph...`
  - You likely ran the wrong command. Rather than running `rover subgraph _`, 
    run `rover graph _`.
  - Your graph may not be setup yet. Make sure at least one subgraph has been
    pushed to the graph before trying again
- `Invalid ChangeSeverity.`
  - This is a result of Apollo Studio returning an improper value during checks.
  - If you see this, please [open an issue](https://github.com/apollographql/rover/issues).

## Feedback

If you are still having issues, or you have seen room for improvement, please
[let us know](./#feedback)!