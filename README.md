<!--- 
# build
target=x86_64-unknown-linux-gnu
export RUSTFLAGS="-C target-feature=-crt-static -C relocation-model=pie"
cargo test --release --target $target \
&& cargo build --release --target $target
--->

[![status](https://github.com/quinlan-lab/bedder-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/quinlan-lab/bedder-rs/actions/) [![documentation](https://img.shields.io/badge/documentation-blue?style=plastic&logoSize=auto)](https://brentp.github.io/bedder-docs/)

# bedder (tools)

This is an early release of the library for feedback, especially from rust practitioners.

## Documentation

Please find documentation [here](https://brentp.github.io/bedder-docs/).

## Problem statement

BEDTools is extremely useful but adding features and maintaining existing ones is a challenge.
We want a library (in bedder) that is:

1. flexible enough to support most use-cases in BEDTools
2. fast enough
3. extensible so that we don't need a custom tool for every possible use-case.

### Solution

To do this, we provide the machinery to intersect common genomics file formats (and more can be added by implementing a simple trait)
and we allow the user to write python functions that then write columns to the output.
As a silly example, the user may want to count overlaps but only if the start position of the overlapping interval is even; that could be
done with this expression:

```python
def bedder_odd(fragment) -> int:
    """return odd if the start of the query interval is odd. otherwise even"""
    return sum(1 for _ in fragment.b if b.start % 2 == 0)
```

There are several things to note here:

+ The function name must start with `bedder_` what follows that will be used as the name in the command-line and as the output e.g. in the VCF INFO field.
+ The function must have a return type annotation (`int`, `str`, `float`, `bool` are supported)
+ The docstring will be used as the description for VCF output, if appropriate
+ The function must accept a fragment--that is, a piece of an alignment.

This function, if placed in a file named `example.py` could be used as:

```bash
bedder -a some.bed -b other.bed -P example.py -c 'py:odd`
```

where `odd` matches the function name above after dropping the `bedder_` prefix.
