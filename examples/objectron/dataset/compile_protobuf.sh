#!/usr/bin/env bash

(cd ./proto && protoc -I . --python_betterproto_out=. ./*.proto)
