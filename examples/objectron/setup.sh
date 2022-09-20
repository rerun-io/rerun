pip install requests "betterproto[compiler]"
(cd ./proto && protoc -I . --python_betterproto_out=. *.proto)
