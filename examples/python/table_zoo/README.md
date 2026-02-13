## Table zoo

A tiny CLI that creates a dummy Arrow table with many datatypes for testing.
By default, it streams the table to a running Rerun Viewer. Alternatively, it can store the table in a local LanceDB and register it with a local server ("register-to-server" mode).


### Example

Send to viewer:

```
python table_zoo.py
```

Register to local server:

```
table_zoo --register-to-server
```
