#!/bin/bash

# This scripts checks if Rover's compiled executable only asks for
# supported versions of glibc

# source: https://gist.github.com/fasterthanlime/17e002a8f5e0f189861c

MAX_VER=2.28

SCRIPTPATH=$( cd $(dirname $0) ; pwd -P )
BINARY="target/debug/rover"

# Version comparison function in bash
vercomp () {
    if [[ $1 == $2 ]]
    then
        return 0
    fi
    local IFS=.
    local i ver1=($1) ver2=($2)
    # fill empty fields in ver1 with zeros
    for ((i=${#ver1[@]}; i<${#ver2[@]}; i++))
    do
        ver1[i]=0
    done
    for ((i=0; i<${#ver1[@]}; i++))
    do
        if [[ -z ${ver2[i]} ]]
        then
            # fill empty fields in ver2 with zeros
            ver2[i]=0
        fi
        if ((10#${ver1[i]} > 10#${ver2[i]}))
        then
            return 1
        fi
        if ((10#${ver1[i]} < 10#${ver2[i]}))
        then
            return 2
        fi
    done
    return 0
}

IFS="
"
VERS=$(objdump -T $BINARY | grep GLIBC | sed 's/.*GLIBC_\([.0-9]*\).*/\1/g' | sort -u)

for VER in $VERS; do
  vercomp $VER $MAX_VER
  COMP=$?
  if [[ $COMP -eq 1 ]]; then
    echo "Error! ${BINARY} requests GLIBC ${VER}, which is higher than target ${MAX_VER}"
    echo "Affected symbols:"
    objdump -T $BINARY | grep GLIBC_${VER}
    echo "Looking for symbols in libraries..."
    for LIBRARY in $(ldd $BINARY | cut -d ' ' -f 3); do
      echo $LIBRARY
      objdump -T $LIBRARY | fgrep GLIBC_${VER}
    done
    exit 27
  else
    echo "Found version ${VER}"
  fi
done
