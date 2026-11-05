#!/usr/bin/env bash
set -euo pipefail

readonly url="$1"
readonly sha256="$2"
readonly output="$3"
readonly attempts=5

for attempt in $(seq "$attempts"); do
  if curl --fail --location --silent --show-error --retry 5 --retry-all-errors --output "$output" "$url" \
    && echo "${sha256}  ${output}" | sha256sum -c -; then
    exit 0
  fi
  echo "Attempt ${attempt}/${attempts} to download ${url} failed" >&2
  if [[ "$attempt" -lt "$attempts" ]]; then
    sleep 10
  fi
done

rm -f "$output"
exit 1
