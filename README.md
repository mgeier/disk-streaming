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

* `cargo build`
* `make`

Running
-------

* Start `jackd` (e.g. with the `qjackctl` tool)
* `export LD_LIBRARY_PATH=target/debug`
* `./example`
* Connect JACK ports to output ports (e.g. with the `qjackctl` tool)
* Play around with the JACK transport (e.g. with the `qjackctl` tool)
