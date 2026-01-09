@echo off
REM Wrapper script to run "uv run python" using the local uv wrapper
REM This ensures we use .venv instead of pixi env

"%~dp0uv.exe" run python %*
