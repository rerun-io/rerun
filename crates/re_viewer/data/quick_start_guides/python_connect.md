## Python Quick Start

${SAFARI_WARNING}

### Installing the Rerun SDK

The Rerun SDK is available on [PyPI](https://pypi.org/) under the
[`rerun-sdk`](https://pypi.org/project/rerun-sdk/) name. It can be installed like any other
Python package:

```sh
pip install rerun-sdk
```

### Try out the viewer

The Rerun SDK comes with a demo that can be used to try the viewer. You can send a demo recording
to this viewer using the following command:

```sh
python -m rerun_sdk --connect
```

This will open a new recording that looks like this:

![Demo recording](https://static.rerun.io/quickstart2_simple_cube/632a8f1c79f70a2355fad294fe085291fcf3a8ae/768w.png)


### Logging your own data

Instead of a pre-packaged demo, you can log your own data. Copy and paste the following snippet in a new Python file and execute it to create a new recording in this viewer:

```python
${EXAMPLE_CODE}
```

${HOW_DOES_IT_WORK}
