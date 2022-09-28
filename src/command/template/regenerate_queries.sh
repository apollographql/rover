#!/usr/bin/env bash

set -euo pipefail

# Install this CLI with `cargo install graphql_client_cli`
graphql-client generate --schema-path schema.graphql queries.graphql \
  --response-derives 'Debug,Serialize,PartialEq,Eq,Clone' \
  --custom-scalars-module crate::command::template::custom_scalars