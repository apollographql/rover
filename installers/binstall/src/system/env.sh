#!/bin/sh
# shell setup
# this file adds the binary to your PATH when it is sourced in a shell profile
# affix colons on either side of $PATH to simplify matching
case ":${PATH}:" in
    *:"{path_to_bin}":*)
        ;;
    *)
        # Prepending path in case a system installed binary must be overwritten
        export PATH="{path_to_bin}:$PATH"
        ;;
esac