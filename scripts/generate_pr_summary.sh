#!/bin/bash

# Exit if GITHUB_SHA is not defined or empty
if [ -z "${GITHUB_SHA}" ]; then
    echo "Error: GITHUB_SHA is not defined or empty."
    exit 1
fi

if [ -z "${PR_NUMBER}" ]; then
    echo "Error: PR_NUMBER is not defined or empty."
    exit 1
fi

GITHUB_SHORT=$(echo $GITHUB_SHA | cut -c1-7)

# Check if the bucket resource exists
gcloud storage ls gs://rerun-web-viewer/commit/${GITHUB_SHORT} > /dev/null 2>&1

if [ $? -eq 0 ]; then
    export HOSTED_APP_URL="https://app.rerun.io/${GITHUB_SHORT}"
else
    export HOSTED_APP_URL=""
fi

# Get the list of wheel files
export WHEELS=$(gcloud storage ls gs://rerun-build/commit/${GITHUB_SHORT}/wheels | grep -oP '(?<=gs://rerun-build/commit/)[^/]+/wheels/wheel\d+.whl')

# Render Jinja template with the HOSTED_APP_URL and WHEELS variables
j2 templates/pr_results_summary.html > build_summary.html
