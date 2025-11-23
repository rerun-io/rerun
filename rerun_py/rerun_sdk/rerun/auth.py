from __future__ import annotations

"""
WorkOS Authentication Module for Jupyter Notebooks.

This module provides OAuth/SSO authentication using WorkOS with browser cookie storage.
"""

import os
import threading
import time
import webbrowser
import requests
import urllib.parse
import secrets
import hashlib
import base64
import uuid
from typing import Optional, Dict, Any, Tuple
from flask import Flask, redirect, request, make_response, jsonify
from IPython.display import display, HTML
from rerun_bindings import OauthLoginFlow, init_login_flow

def login_with_browser() -> None:
    """Initiate OAuth flow by redirecting to WorkOS authorization URL."""

    flow = init_login_flow()

    if flow is None:
        print("Already logged in, skipping login flow.")
        return

    # Automatically open the login URL in the default browser
    login_url = flow.login_url()
    print(f"Please open the following URL in your browser if it doesn't open automatically: {login_url}")
    webbrowser.open(login_url)

    # Wait for the flow to complete and store credentials
    flow.finish_login_flow()

