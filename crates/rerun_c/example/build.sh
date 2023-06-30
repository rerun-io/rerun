#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path"

if [ ! -z ${1+x} ] && [ $1 == "clean" ]; then
    rm -rf build
    exit 0
fi

cargo build -p rerun_c

mkdir -p build

CXX=g++
CPPFLAGS="--std=c++14 -Wall -Wno-sign-compare -O2 -g -DNDEBUG"
LDLIBS="-lstdc++ -lpthread -ldl"
LDLIBS="$LDLIBS -framework CoreFoundation -framework IOKit" # TODO: only mac
CPPFLAGS="$CPPFLAGS -I ../src" # Make sure rerun.h is found
OBJECTS="../../../target/debug/librerun_c.a" # TODO: support non-Mac

for source_path in *.cpp; do
    obj_path="build/${source_path%.cpp}.o"
    OBJECTS="$OBJECTS $obj_path"
    if [ ! -f $obj_path ] || [ $obj_path -ot $source_path ]; then
        echo "Compiling $source_path to $obj_path..."
        $CXX $CPPFLAGS -c $source_path -o $obj_path
    fi
done

echo "Linking..."
$CXX $CPPFLAGS $OBJECTS $LDLIBS -o example.bin
