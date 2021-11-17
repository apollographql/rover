---
title: "Using Rover with a Proxy Server"
sidebar_title: "Proxy Configuration"
---

## Overview

If you have an HTTP or SOCKS5 proxy server on your network between a host running Rover and Apollo's endpoints, you must set the `HTTP_PROXY` environment variable to the hostname or IP address of the proxy server. If you are using a secure proxy server, you can instead set `HTTPS_PROXY`.

`HTTP(S)_PROXY` is a standard environment variable. Like any environment variable, the specific steps you use to set it depends on your operating system.

## Example

On the same line:
```shell
HTTPS_PROXY=socks5://127.0.0.1:1086 \
    rover graph check my-company@prod --profile work
```

or

```shell
export HTTPS_PROXY=socks5://127.0.0.1:1086
rover graph check my-company@prod --profile work
```

## Force bypass proxy

If you have the `HTTP(S)_PROXY` environment variable set in your environment but you Rover to bypass the proxy, set the `NO_PROXY` environment variable to `true`.