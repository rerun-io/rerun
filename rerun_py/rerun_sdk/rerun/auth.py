from __future__ import annotations

from IPython.core.display import HTML
from IPython.display import display

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
    print("Click the button to login in your browser:")
    display(HTML(f'<a href="{login_url}" target="_blank" style="">Login</a>'))
    print("You should see the following code:")
    print(flow.user_code())

    # Wait for the flow to complete and store credentials
    credentials = flow.finish_login_flow()
    print(f"Success! You're logged in as '{credentials.user_email}'.")
