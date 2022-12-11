cargo install cargo-cranky
cargo install cargo-deny
cargo install taplo-cli --locked

REM Note that as of writing building maturin from source doesn't work on aarch64.
REM Instead, download a binary manually from https://github.com/PyO3/maturin/releases/
cargo install maturin


REM Other software that needs to be installed:
REM - cmake
REM -
