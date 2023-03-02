#!/usr/bin/env bash
# Check for files that are too large to be checked into the repository.
# Whenever we want to make an exception, we add it to `check_large_files_allow_list.txt`
set -eu

# 300KiB maximum size.
maximum_size=$((1024 * 300))

result=0
while read -d '' -r file; do
    if [[ -f "$file" ]]; then
        actualsize=$(wc -c <"$file")
        if [ $actualsize -ge $maximum_size ]; then
            if ! grep -qx "$file" ./scripts/check_large_files_allow_list.txt; then
                echo $file is larger than $maximum_size bytes
                result=1
            fi
        fi
    fi
done < <(git ls-files -z --empty-directory)

exit $result
