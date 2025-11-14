# build static binary

cd bedder-rs

## build the container

```
docker build -f docker/Dockerfile.gnu-static -t bedder-static .
```

## build the binary in the container

```
docker run -it -p 1455:1455 -v $(pwd):/workspace/bedder-rs  bedder-static bash  docker/build-static.sh
```

output will be in **dist/**

That binary will still require python libraries (but not libpython). If you see:

```
Could not find platform independent libraries <prefix>
Could not find platform dependent libraries <exec_prefix>
Fatal Python error: Failed to import encodings module
Python runtime state: core initialized
ModuleNotFoundError: No module named 'encodings'

Current thread 0x0000000040a43540 (most recent call first):
  <no Python frame>
```

This means it can't find your python libraries. You can activate a uv (or pip) venv or set PYTHONPATH, e.g.

```
PYTHONPATH=~/miniforge3/lib/python3.10/
```

## can't find libpython.so

For this error:

```
error while loading shared libraries: libpython3.13.so.1.0: cannot open shared object file: No such file or directory
```

you are using a binary that does not have python linked staticly so you must set LD_LIBRARY_PATH to a directory that contains the `.so` files mentioned in the error message.
