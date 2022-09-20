## TODO: should this really be here?
pip install requests "betterproto[compiler]" pillow scipy
(cd ./proto && protoc -I . --python_betterproto_out=. *.proto)
