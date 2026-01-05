@echo off
REM Wrapper script to run uv without CONDA_PREFIX so it uses .venv instead of pixi env
REM This script is prepended to PATH by pixi activation so it shadows the real uv

setlocal

REM Remove CONDA_PREFIX from environment
set CONDA_PREFIX=

REM Find and run the real uv (from pixi's conda env)
REM We need to call the uv.exe that's in the pixi env bin directory
"%PIXI_PROJECT_ROOT%\.pixi\envs\default\Scripts\uv.exe" %*
