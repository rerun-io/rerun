from __future__ import annotations

from rerun_bindings import (
    Credentials as Credentials,
    get_credentials as get_credentials,
    init_login_flow,
    logout as _logout_bindings,
)

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


def logout() -> None:
    """Log out from OAuth session."""

    import webbrowser

    logout_url = _logout_bindings()
    if logout_url is not None:
        webbrowser.open(logout_url)
        print("You have been logged out.")
    else:
        print("No credentials found. You are already logged out.")
