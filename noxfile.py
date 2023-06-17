from __future__ import annotations

import nox  # type: ignore


@nox.session(python=["3.8", "3.9"])
def one_example(session: nox.Session) -> None:
    session.install("-r", "examples/python/car/requirements.txt")
    # session.install("rerun_py")
    session.run("maturin", "develop", "--manifest-path", "rerun_py/Cargo.toml", '--extras="tests"')
    session.run("python", "examples/python/car/main.py", "--save", "/tmp/out.rrd")
