# source this file to set up the python environment

python3 -m venv env
source env/bin/activate
python3 -m pip install --upgrade pip
python3 -m pip install -r crates/re_sdk_python/requirements-build.txt
