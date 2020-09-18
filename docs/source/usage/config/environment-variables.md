---
title: "Environment Variables"
sidebar_title: "Environment Variables"
description: "Setting up environment variables to use with the Apollo CLI"
---

The `rover` CLI recognizes several environment variables, which better allow
you to leverage this tool in CI and/or override defaults and pre-configured
values.

Environment variables have the highest precedence of all configuration. This
means that the value of an environment value will always override other
configuration.

| Name               | Value                                              | Default                             | Notes                                                                                                                   |
|--------------------|----------------------------------------------------|-------------------------------------|-------------------------------------------------------------------------------------------------------------------------|
| APOLLO_CONFIG_HOME | Path to where CLI configuration should be stored.  | Default OS Configuration directory. | More information in [Authentication - Configuration](./usage/config/authentication.html#configuration).                  |
| APOLLO_KEY         | An API Key generated from Apollo Studio.           | none                                | More information in [Authentication - Environment Variables](./usage/config/authentication.html#environment-variables). |
