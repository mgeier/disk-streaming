Loading and Decoding Audio Data from Disk
=========================================

This is a proof-of-concept implementation.
If successful, it will be incorporated in a Rust implementation of the ASDF
(https://github.com/AudioSceneDescriptionFormat).

Requirements
------------

* Rust compiler, Cargo (https://rustup.rs/)
* JACK (http://jackaudio.org/)
* A C++ compiler, Make

Compilation
-----------

    cargo build --all

Example C++ progam:

    cd examples
    make

Running
-------

* Start `jackd` (e.g. with the `qjackctl` tool)
* `cd examples`
* `export LD_LIBRARY_PATH=../target/debug`
* `./example`
* Connect JACK ports to output ports (e.g. with the `qjackctl` tool)
* Play around with the JACK transport (e.g. with the `qjackctl` tool)

Updating the C Header File
--------------------------

The file `disk_streaming.h` was generated with
[cbindgen](https://crates.io/crates/cbindgen) (`cargo install cbindgen`).
After changes in the API functions, it can be updated with

* `cbindgen ffi -o ffi/disk_streaming.h`
