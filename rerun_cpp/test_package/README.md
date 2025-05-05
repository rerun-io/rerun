# Test Package for Rerun SDK

This test package validates that the Rerun SDK Conan package can be properly consumed in a project.

## What it tests

The example demonstrates:
- Finding the Rerun SDK package with CMake
- Linking against the library
- Basic usage of the Rerun SDK API

## Running the test

The test will be automatically run when you create the Rerun SDK package with:

```bash
conan create .
```

Or explicitly test an existing package with:

```bash
conan test test_package rerun-sdk/version@user/channel
```
