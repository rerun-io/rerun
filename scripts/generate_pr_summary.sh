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

export GITHUB_SHORT=$(echo $GITHUB_SHA | cut -c1-7)

# Check if the bucket resource exists
gcloud storage ls gs://rerun-web-viewer/commit/${GITHUB_SHORT} > /dev/null 2>&1

if [ $? -eq 0 ]; then
    export HOSTED_APP_URL="https://app.rerun.io/commit/${GITHUB_SHORT}"
else
    export HOSTED_APP_URL=""
fi

# Get the list of wheel files
export WHEELS=$(gcloud storage ls gs://rerun-builds/commit/${GITHUB_SHORT}/wheels | awk -F/ '{print $NF}' | grep -E '.whl$')
export WHEEL_BASE_URL="https://storage.googleapis.com/rerun-builds/commit/${GITHUB_SHORT}/wheels"

# Render Jinja template with the HOSTED_APP_URL and WHEELS variables
j2 templates/pr_results_summary.html > build_summary.html
