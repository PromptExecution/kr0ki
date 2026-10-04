#!/bin/sh
# Verify the default compiler and Cargo that build the stable generator.
set -eu
expected=${1:?expected stable toolchain version is required}
compiler=$(rustc -vV | sed -n 's/^release: //p')
cargo_version=$(cargo --version | cut -d ' ' -f 2)
if [ "$compiler" != "$expected" ] || [ "$cargo_version" != "$expected" ]; then
    echo "docgen base toolchain mismatch: expected rustc/cargo $expected; found rustc $compiler, cargo $cargo_version" >&2
    exit 1
fi
