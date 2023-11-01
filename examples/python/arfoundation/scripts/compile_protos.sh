#!/bin/sh

python -m grpc_tools.protoc -Iprotos --python_out=. --pyi_out=. --grpc_python_out=. protos/*.proto
protoc -Iprotos --csharp_out=client/Assets/Scripts/RerunAR --grpc_csharp_out=client/Assets/Scripts/RerunAR protos/*.proto
