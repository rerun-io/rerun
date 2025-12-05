from __future__ import annotations

from rerun_bindings import init_login_flow

"""
WorkOS Authentication Module for Jupyter Notebooks.

This module provides OAuth/SSO authentication using WorkOS.
"""


def login() -> None:
    """Initiate OAuth flow by redirecting to WorkOS authorization URL."""

    flow = init_login_flow()

    if flow is None:
        print("Already logged in, skipping login flow.")
        return

    # Automatically open the login URL in the default browser
    login_url = flow.login_url()
    print("Open the following URL in your browser:")
    print()
    print(f"  {login_url}")
    print()
    print("You should see the following code:")
    print()
    print(f"  {flow.user_code()}")

    # Wait for the flow to complete and store credentials
    credentials = flow.finish_login_flow()
    print()
    print(f"Success! You're logged in as '{credentials.user_email}'.")
