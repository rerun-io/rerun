"""
WorkOS Authentication Module for Jupyter Notebooks

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
from typing import Optional, Dict, Any, Tuple
from flask import Flask, redirect, request, make_response, jsonify
from IPython.display import display, HTML


class WorkOSAuth:
    """Handle WorkOS OAuth authentication flow in Jupyter environments."""

    def __init__(
        self,
    ):
        """
        Initialize WorkOS authentication handler.

        Args:
            client_id: WorkOS Client ID (or set RERUN_OAUTH_CLIENT_ID env var)
            redirect_uri: OAuth redirect URI (default: http://localhost:{port}/callback)
            port: Port for Flask callback server (default: 5000)
        """

        self.oauth_server_url = os.getenv("RERUN_OAUTH_SERVER_URL") or "https://api.workos.com/user_management"
        self.client_id = os.getenv("RERUN_OAUTH_CLIENT_ID") or "client_01JZ3JVQW6JNVXME6HV9G4VR0H"
        self.code_verifier, self.code_challenge = self.generate_pkce_pair()
        self.port = 5001
        self.redirect_uri = f"http://127.0.0.1:{self.port}/login-callback"

        if not self.client_id:
            raise ValueError("OAuth2 client ID is required.")

        # Flask app for OAuth callback
        self.app = Flask(__name__)
        self.app.secret_key = os.urandom(24)
        self.server_thread = None
        self.user_profile = None
        self.access_token = None
        self.refresh_token = None
        self.organization_id = os.getenv("RERUN_OAUTH_ORGANIZATION_ID") or "org_01K6MTXXG0YSAFP6Q7AREM0DJQ"

        redirect_url_safe = urllib.parse.quote(self.redirect_uri, safe="")

        self.authorization_url = (
            f"{self.oauth_server_url}/authorize"
            f"?client_id={self.client_id}"
            f"&redirect_uri={redirect_url_safe}"
            f"&response_type=code"
            f"&organization_id={self.organization_id}"
            f"&code_challenge={self.code_challenge}"
            f"&code_challenge_method=S256"
        )
        print(f"Authorization URL: {self.authorization_url}")

        # Setup Flask routes
        self._setup_routes()

    def generate_pkce_pair(self) -> Tuple[str, str]:
        code_verifier = secrets.token_urlsafe(96)[:128]
        hashed = hashlib.sha256(code_verifier.encode("ascii")).digest()
        encoded = base64.urlsafe_b64encode(hashed)
        code_challenge = encoded.decode("ascii")[:-1]
        return code_verifier, code_challenge

    def _setup_routes(self):
        """Setup Flask routes for OAuth flow."""

        @self.app.route("/")
        def index():
            """Home page showing authentication status."""
            auth_cookie = request.cookies.get("workos_auth_token")
            profile_cookie = request.cookies.get("workos_user_profile")

            if auth_cookie and profile_cookie:
                return f"""
                <html>
                <head><title>WorkOS Authentication</title></head>
                <body style="font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px;">
                    <h1>✓ Authenticated</h1>
                    <p>You are successfully logged in!</p>
                    <p><strong>User Profile:</strong> {profile_cookie}</p>
                    <p><a href="/logout">Logout</a></p>
                </body>
                </html>
                """
            else:
                return f"""
                <html>
                <head><title>WorkOS Authentication</title></head>
                <body style="font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px;">
                    <h1>WorkOS Authentication</h1>
                    <p>Click the button below to authenticate via WorkOS SSO:</p>
                    <p><a href="/login" style="display: inline-block; background: #6C5CE7; color: white; padding: 10px 20px; text-decoration: none; border-radius: 5px;">Login with WorkOS</a></p>
                </body>
                </html>
                """

        @self.app.route("/login")
        def login():
            """Initiate OAuth flow by redirecting to WorkOS authorization URL."""
            try:
                return redirect(self.authorization_url)
            except Exception as e:
                return (
                    f"""
                <html>
                <body style="font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px;">
                    <h1>Authentication Error</h1>
                    <p style="color: red;">Error initiating authentication: {str(e)}</p>
                    <p><a href="/">Go back</a></p>
                </body>
                </html>
                """,
                    500,
                )

        @self.app.route("/login-callback")
        def callback():
            """Handle OAuth callback from WorkOS."""
            try:
                # Get authorization code from query parameters
                code = request.args.get("code")

                if not code:
                    return "Error: No authorization code received", 400

                print(f"Code: {code}")
                auth_raw_response = requests.post(
                    f"{self.oauth_server_url}/authenticate",
                    json={
                        "code": code,
                        "code_verifier": self.code_verifier,
                        "grant_type": "authorization_code",
                        "client_id": self.client_id,
                    },
                )
                print(f"Auth response: {auth_raw_response}")

                auth_response = auth_raw_response.json()
                print(f"Auth response JSON: {auth_response}")

                profile = auth_response["user"]
                self.access_token = auth_response["access_token"]
                self.refresh_token = auth_response["refresh_token"]

                # Create response with cookies
                response = make_response(f"""
                <html>
                <head>
                    <title>Authentication Successful</title>
                    <meta http-equiv="refresh" content="2;url=/">
                </head>
                <body style="font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px;">
                    <h1>✓ Authentication Successful!</h1>
                    <p>Welcome, {profile["email"] if profile["email"] is not None else profile["id"]}!</p>
                    <p>Redirecting to home page...</p>
                    <p><a href="/">Continue</a></p>
                </body>
                </html>
                """)

                # Set authentication cookies
                response.set_cookie(
                    "workos_auth_token",
                    self.access_token,
                    httponly=True,
                    secure=False,  # Set to True in production with HTTPS
                    samesite="Lax",
                    max_age=3600 * 24 * 7,  # 7 days
                )

                # Store minimal user profile info in cookie
                user_info = {
                    "id": profile["id"],
                    "email": profile["email"],
                    "first_name": profile["first_name"],
                    "last_name": profile["last_name"],
                }

                response.set_cookie(
                    "workos_user_profile",
                    str(user_info),
                    httponly=False,  # Allow JavaScript access for display
                    secure=False,
                    samesite="Lax",
                    max_age=3600 * 24 * 7,
                )

                self.user_profile = user_info
                self.auth_token = self.access_token

                return response

            except Exception as e:
                return (
                    f"""
                <html>
                <body style="font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px;">
                    <h1>Authentication Error</h1>
                    <p style="color: red;">Error during authentication: {str(e)}</p>
                    <p><a href="/">Go back</a></p>
                </body>
                </html>
                """,
                    500,
                )

        @self.app.route("/logout")
        def logout():
            """Clear authentication cookies and logout."""
            response = make_response(f"""
            <html>
            <head>
                <title>Logged Out</title>
                <meta http-equiv="refresh" content="2;url=/">
            </head>
            <body style="font-family: Arial, sans-serif; max-width: 600px; margin: 50px auto; padding: 20px;">
                <h1>Logged Out</h1>
                <p>You have been successfully logged out.</p>
                <p>Redirecting to home page...</p>
                <p><a href="/">Continue</a></p>
            </body>
            </html>
            """)

            # Clear cookies
            response.set_cookie("workos_auth_token", "", expires=0)
            response.set_cookie("workos_user_profile", "", expires=0)

            return response

        @self.app.route("/status")
        def status():
            """Return authentication status as JSON."""
            auth_cookie = request.cookies.get("workos_auth_token")
            profile_cookie = request.cookies.get("workos_user_profile")

            return jsonify({
                "authenticated": bool(auth_cookie),
                "profile": profile_cookie if profile_cookie else None,
            })

    def start_server(self, background: bool = True, open_browser: bool = False):
        """
        Start the Flask authentication server.

        Args:
            background: Run server in background thread (default: True)
            open_browser: Automatically open browser to login page (default: False)
        """
        if self.server_thread and self.server_thread.is_alive():
            print(f"⚠️  Server already running on http://localhost:{self.port}")
            return

        def run_server():
            self.app.run(host="localhost", port=self.port, debug=False, use_reloader=False)

        if background:
            self.server_thread = threading.Thread(target=run_server, daemon=True)
            self.server_thread.start()
            time.sleep(1)  # Give server time to start
            print(f"✓ Authentication server started on http://localhost:{self.port}")

            # wait for the server to respond to the status endpoint
            attempts = 50
            while attempts > 0:
                print(f"Waiting for server to start... {attempts} attempts left")
                response = requests.get(f"http://localhost:{self.port}/status")
                print(f"Status response: {response.status_code}")
                if response.status_code == 200:
                    break
                time.sleep(0.1)
                attempts -= 1
            if attempts <= 0:
                raise Exception("Failed to start server")

        else:
            run_server()

        if open_browser:
            webbrowser.open(f"http://localhost:{self.port}/login")

    def display_login_link(self):
        """Display a clickable login link in Jupyter notebook."""
        login_url = f"http://localhost:{self.port}/login"
        html = f"""
        <div style="padding: 20px; background: #f5f5f5; border-radius: 5px; margin: 10px 0;">
            <h3 style="margin-top: 0;">WorkOS Authentication</h3>
            <p>Click the button below to authenticate:</p>
            <a href="{login_url}" target="_blank" style="display: inline-block; background: #6C5CE7; color: white; padding: 12px 24px; text-decoration: none; border-radius: 5px; font-weight: bold;">
                Login with WorkOS
            </a>
            <p style="margin-top: 15px; font-size: 0.9em; color: #666;">
                Or visit: <a href="{login_url}" target="_blank">{login_url}</a>
            </p>
        </div>
        """
        display(HTML(html))

    def get_status(self) -> Dict[str, Any]:
        """
        Get current authentication status.

        Returns:
            Dictionary with authentication status and user profile if available
        """
        return {
            "authenticated": self.user_profile is not None,
            "profile": self.user_profile,
            "token": self.auth_token is not None,
        }

    def do_refresh_token(self):
        """Refresh the authentication token."""
        auth_raw_response = requests.post(
            "https://api.workos.com/user_management/authenticate",
            json={
                "refresh_token": self.refresh_token,
                "grant_type": "refresh_token",
                "client_id": self.client_id,
            },
        )
        auth_response = auth_raw_response.json()
        print(f"Auth response: {auth_response}")

        self.access_token = auth_response["access_token"]
        self.refresh_token = auth_response["refresh_token"]

        return auth_response

    def stop_server(self):
        """Stop the Flask server (note: may require kernel restart for full cleanup)."""
        print("⚠️  Note: Flask server is running in a daemon thread. To fully stop it, restart the Jupyter kernel.")
