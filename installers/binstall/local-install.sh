#!/bin/bash
#
# Licensed under the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.

# This script is like install.sh except it uses binaries from
# your local machine so it's easier to test/develop.

set -u

copy_binary_and_run_installer() {
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir
    need_cmd tar
    need_cmd which
    need_cmd dirname
    need_cmd cargo
    need_cmd tput

    say "building rover"
    ensure cargo build --workspace
    say "build complete"

    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    say "$_arch"

    local _ext=""
    case "$_arch" in
        *windows*)
            _ext=".exe"
            ;;
    esac

    local _dir="./target/debug"
    local _rover="$_dir/rover$_ext"

    # The installer may want to ask for confirmation on stdin for various
    # operations. We were piped through `sh` though so we probably don't have
    # access to a tty naturally. If it looks like we're attached to a terminal
    # (`-t 1`) then pass the tty down to the installer explicitly.
    if [ -t 1 ]; then
      "$_rover" "install" "$@" < /dev/tty
    else
      "$_rover" "install" "$@"
    fi

    local _retval=$?

    return "$_retval"
}

get_architecture() {
    local _ostype="$(uname -s)"
    local _cputype="$(uname -m)"

    if [ "$_ostype" = Darwin -a "$_cputype" = i386 ]; then
        # Darwin `uname -s` lies
        if sysctl hw.optional.x86_64 | grep -q ': 1'; then
            local _cputype=x86_64
        fi
    fi

    case "$_ostype" in
        Linux)
            local _ostype=unknown-linux-musl
            ;;

        Darwin)
            local _ostype=apple-darwin
            ;;

        MINGW* | MSYS* | CYGWIN*)
            local _ostype=pc-windows-msvc
            ;;

        *)
            err "no precompiled binaries available for OS: $_ostype"
            ;;
    esac

    case "$_cputype" in
        x86_64 | x86-64 | x64 | amd64)
            local _cputype=x86_64
            ;;
        *)
            err "no precompiled binaries available for CPU architecture: $_cputype"

    esac

    local _arch="$_cputype-$_ostype"

    RETVAL="$_arch"
}


say() {
    local green=`tput setaf 2`
    local reset=`tput sgr0`
    echo "  ${green}INFO${reset} sh::wrapper: $1"
}

err() {
    local red=`tput setaf 1`
    local reset=`tput sgr0`
    say "  ${red}ERROR${reset} sh::wrapper: $1" >&2
    exit 1
}

need_cmd() {
    if ! check_cmd "$1"
    then err "need '$1' (command not found)"
    fi
}

check_cmd() {
    command -v "$1" > /dev/null 2>&1
    return $?
}

need_ok() {
    if [ $? != 0 ]; then err "$1"; fi
}

assert_nz() {
    if [ -z "$1" ]; then err "assert_nz $2"; fi
}

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing
# command.
ensure() {
    "$@"
    need_ok "command failed: $*"
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
    "$@"
}

copy_binary_and_run_installer "$@" || exit 1