# E2E REDAP Tests

End-to-end test suite for redap (Rerun Data Protocol).

## Overview

This test suite exercises the full redap stack by using the Pythion SDK (primarily `CatalogClient`) against a live Rerun server.

## Architectural notes

The `catalog_client` fixture is the foundation of this test suite. It yields a connected `CatalogClient` instance which all other fixtures and tests depend on.

