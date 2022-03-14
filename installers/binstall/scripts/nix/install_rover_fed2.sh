#!/bin/bash
#
# Copyright 2021 Apollo Graph, Inc.

# Elastic License 2.0

# ## Acceptance

# By using the software, you agree to all of the terms and conditions below.

# ## Copyright License

# The licensor grants you a non-exclusive, royalty-free, worldwide,
# non-sublicensable, non-transferable license to use, copy, distribute, make
# available, and prepare derivative works of the software, in each case subject to
# the limitations and conditions below.

# ## Limitations

# You may not provide the software to third parties as a hosted or managed
# service, where the service provides users with access to any substantial set of
# the features or functionality of the software.

# You may not move, change, disable, or circumvent the license key functionality
# in the software, and you may not remove or obscure any functionality in the
# software that is protected by the license key.

# You may not alter, remove, or obscure any licensing, copyright, or other notices
# of the licensor in the software. Any use of the licensor’s trademarks is subject
# to applicable law.

# ## Patents

# The licensor grants you a license, under any patent claims the licensor can
# license, or becomes able to license, to make, have made, use, sell, offer for
# sale, import and have imported the software, in each case subject to the
# limitations and conditions in this license. This license does not cover any
# patent claims that you cause to be infringed by modifications or additions to
# the software. If you or your company make any written claim that the software
# infringes or contributes to infringement of any patent, your patent license for
# the software granted under these terms ends immediately. If your company makes
# such a claim, your patent license ends immediately for work on behalf of your
# company.

# ## Notices

# You must ensure that anyone who gets a copy of any part of the software from you
# also gets a copy of these terms.

# If you modify the software, you must include in any modified copies of the
# software prominent notices stating that you have modified the software.

# ## No Other Rights

# These terms do not imply any licenses other than those expressly granted in
# these terms.

# ## Termination

# If you use the software in violation of these terms, such use is not licensed,
# and your licenses will automatically terminate. If the licensor provides you
# with a notice of your violation, and you cease all violation of this license no
# later than 30 days after you receive that notice, your licenses will be
# reinstated retroactively. However, if you violate these terms after such
# reinstatement, any additional violation of these terms will cause your licenses
# to terminate automatically and permanently.

# ## No Liability

# *As far as the law allows, the software comes as is, without any warranty or
# condition, and the licensor will not be liable to you for any damages arising
# out of these terms or the use or nature of the software, under any kind of
# legal claim.*

# ## Definitions

# The **licensor** is the entity offering these terms, and the **software** is the
# software the licensor makes available under these terms, including any portion
# of it.

# **you** refers to the individual or entity agreeing to these terms.

# **your company** is any legal entity, sole proprietorship, or other kind of
# organization that you work for, plus all organizations that have control over,
# are under the control of, or are under common control with that
# organization. **control** means ownership of substantially all the assets of an
# entity, or the power to direct its management and policies by vote, contract, or
# otherwise. Control can be direct or indirect.

# **your licenses** are all the licenses granted to you for the software under
# these terms.

# **use** means anything you do with the software requiring one of your licenses.

# **trademark** means trademarks, service marks, and similar rights.

# --------------------------------------------------------------------------------

# This is just a little script that can be downloaded from the internet to
# install rover-fed2. You must first install Rover and have it in your PATH
# in order to use this script. If you have installed via npm, you will need to uninstall
# and reinstall Rover with the curl | sh installer.

# This script first downloads Rover, which in turn downloads the rover-fed2 tarball
# by invoking `rover install --plugin rover-fed2 $Args`. This means that you can pass
# arguments to this shell script and they will be passed along to the installer.

# Example to bypass binary overwrite [y/N] prompt
# curl -sSL https://rover.apollo.dev/plugins/rover-fed2/nix/latest | sh -s -- --force

# Example to Accept the terms and conditions in the elv2 license
# curl -sSL https://rover.apollo.dev/plugins/rover-fed2/win/latest | sh -s -- --elv2-license accept

set -u

BINARY_DOWNLOAD_PREFIX="https://github.com/apollographql/rover/releases/download"

# Rover version defined in root cargo.toml
# Note: this line is built automatically
# in build.rs. Don't touch it!
PACKAGE_VERSION="v0.4.6"

download_binary_and_run_installer() {
    downloader --check
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd rmdir
    need_cmd tar
    need_cmd which
    need_cmd dirname
    need_cmd awk
    need_cmd cut

    # if $VERSION isn't provided or has 0 length, use version from Rover cargo.toml
    # ${VERSION:-} checks if version exists, and if doesn't uses the default
    # which is after the :-, which in this case is empty. -z checks for empty str
    if [ -z ${VERSION:-} ]; then
        # VERSION is either not set or empty
        DOWNLOAD_VERSION=$PACKAGE_VERSION
    else
        # VERSION set and not empty
        DOWNLOAD_VERSION=$VERSION
    fi


    get_architecture || return 1
    local _arch="$RETVAL"
    assert_nz "$_arch" "arch"

    local _ext=""
    case "$_arch" in
        *windows*)
            _ext=".exe"
            ;;
    esac

    local _tardir="rover-$DOWNLOAD_VERSION-${_arch}"
    local _url="$BINARY_DOWNLOAD_PREFIX/$DOWNLOAD_VERSION/${_tardir}.tar.gz"
    local _dir="$(mktemp -d 2>/dev/null || ensure mktemp -d -t rover)"
    local _file="$_dir/input.tar.gz"
    local _rover="$_dir/rover$_ext"

    say "downloading rover from $_url" 1>&2

    ensure mkdir -p "$_dir"
    downloader "$_url" "$_file"
    if [ $? != 0 ]; then
      say "failed to download $_url"
      say "this may be a standard network error, but it may also indicate"
      say "that rover's release process is not working. When in doubt"
      say "please feel free to open an issue!"
      say "https://github.com/apollographql/rover/issues/new/choose"
      exit 1
    fi

    ensure tar xf "$_file" --strip-components 1 -C "$_dir"

    # The installer may want to ask for confirmation on stdin for various
    # operations. We were piped through `sh` though so we probably don't have
    # access to a tty naturally. If it looks like we're attached to a terminal
    # (`-t 1`) then pass the tty down to the installer explicitly.
    if [ -t 1 ]; then
      "$_rover" "install" "--plugin" "rover-fed2" "$@" < /dev/tty
    else
      "$_rover" "install" "--plugin" "rover-fed2" "$@"
    fi

    local _retval=$?

    ignore rm -rf "$_dir"

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

    if [ "$_ostype" = Darwin -a "$_cputype" = arm64 ]; then
        # Darwin `uname -s` doesn't seem to lie on Big Sur
        # but we want to serve x86_64 binaries anyway so that they can
        # then run in x86_64 emulation mode on their arm64 devices
        local _cputype=x86_64
    fi

    case "$_ostype" in
        Linux)
            if has_required_glibc; then
                local _ostype=unknown-linux-gnu
            fi
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
            ;;
        *)
            err "no precompiled binaries available for CPU architecture: $_cputype"

    esac

    local _arch="$_cputype-$_ostype"

    RETVAL="$_arch"
}

say() {
    local green=`tput setaf 2 2>/dev/null || echo ''`
    local reset=`tput sgr0 2>/dev/null || echo ''`
    echo "$1"
}

err() {
    local red=`tput setaf 1 2>/dev/null || echo ''`
    local reset=`tput sgr0 2>/dev/null || echo ''`
    say "${red}ERROR${reset}: $1" >&2
    exit 1
}

has_required_glibc() {
    local _ldd_version="$(ldd --version 2>&1 | head -n1)"
    # glibc version string is inconsistent across distributions
    # instead check if the string does not contain musl (case insensitive)
    if echo "${_ldd_version}" | grep -iv musl >/dev/null; then
        local _glibc_version=$(echo "${_ldd_version}" | awk 'NR==1 { print $NF }')
        local _glibc_major_version=$(echo "${_glibc_version}" | cut -d. -f1)
        local _glibc_min_version=$(echo "${_glibc_version}" | cut -d. -f2)
        local _min_major_version=2
        local _min_minor_version=17
        if [ "${_glibc_major_version}" -gt "${_min_major_version}" ] \
            || { [ "${_glibc_major_version}" -eq "${_min_major_version}" ] \
            && [ "${_glibc_min_version}" -ge "${_min_minor_version}" ]; }; then
            return 0
        else
            say "This operating system needs glibc >= ${_min_major_version}.${_min_minor_version}, but only has ${_libc_version} installed."
        fi
    else
        say "This operating system does not support dynamic linking to glibc."
    fi

    return 1
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

# This wraps curl or wget. Try curl first, if not installed,
# use wget instead.
downloader() {
    if check_cmd curl
    then _dld=curl
    elif check_cmd wget
    then _dld=wget
    else _dld='curl or wget' # to be used in error message of need_cmd
    fi

    if [ "$1" = --check ]
    then need_cmd "$_dld"
    elif [ "$_dld" = curl ]
    then curl -sSfL "$1" -o "$2"
    elif [ "$_dld" = wget ]
    then wget "$1" -O "$2"
    else err "Unknown downloader"   # should not reach here
    fi
}

download_binary_and_run_installer "$@" || exit 1
