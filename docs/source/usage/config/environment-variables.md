---
title: "Environment Variables"
sidebar_title: "Environment Variables"
description: "Setting up environment variables to use with Rover"
---

The `rover` CLI recognizes several environment variables, which better allow
you to leverage this tool in CI and/or override defaults and pre-configured
values.

Environment variables have the highest precedence of all configuration. This
means that the value of an environment value will always override other
configuration.

| Name               | Value                                              | Default                             | Notes                                                                                                                   |
|--------------------|----------------------------------------------------|-------------------------------------|-------------------------------------------------------------------------------------------------------------------------|
| `APOLLO_CONFIG_HOME` | Path to where CLI configuration is stored.  | Default OS Configuration directory. | More information in [Authentication - Configuration](./authentication#configuration).                  |
| `APOLLO_KEY`         | An API Key generated from Apollo Studio.           | none                                | More information in [Authentication - Environment Variables](./authentication#environment-variables). |
| `APOLLO_TELEMETRY_URL` | The URL where anonymous usage data is reported | [https://install.apollographql.com/telemetry](https://install.apollographql.com/telemetry) | More information in [Privacy](../../privacy). |
| `APOLLO_TELEMETRY_DISABLED` | Set to `1` if you don't want Rover to collect anonymous usage data | none | |
