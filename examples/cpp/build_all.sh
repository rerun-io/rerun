#!/usr/bin/env bash

set -eu
script_path=$( cd "$(dirname "${BASH_SOURCE[0]}")" ; pwd -P )
cd "$script_path/../.."
set -x


WERROR=false

while test $# -gt 0; do
  case "$1" in
    --werror)
      shift
      WERROR=true
      ;;

    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done


num_threads=$(getconf _NPROCESSORS_ONLN)

mkdir -p build
pushd build
    if [ ${WERROR} = true ]; then
        cmake -DCMAKE_BUILD_TYPE=Debug -DCMAKE_COMPILE_WARNING_AS_ERROR=ON ..
    else
        cmake -DCMAKE_BUILD_TYPE=Debug ..
    fi
    cmake --build . --config Debug --target examples -j ${num_threads}
popd
