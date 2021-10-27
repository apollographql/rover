---
title: "Using Rover with a Proxy Server"
sidebar_title: "With a Proxy Server"
---

## Overview

If you have an HTTP or SOCKS5 proxy server on your network between a host running Rover and Apollo's endpoints, you must set `HTTP_PROXY` or `http_proxy` with the hostname or IP address of the proxy server. If you are using a secure proxy server, you can set instead `HTTPS_PROXY` or `https_proxy`.

`http(s)_proxy` is a standard environment variable. Like any environment variable, the specific steps you use to set it depends on your operating system.

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

If you have `http(s)_proxy` environment variable initialised but you want to bypass your proxy server, set `NO_PROXY` environment variable to `true`.