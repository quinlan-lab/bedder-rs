#!/usr/bin/env bash
set -euo pipefail

# Build and publish Python-friendly API docs for the PyO3 module
# Requires: Python 3.10+ and `uv` on PATH (https://github.com/astral-sh/uv)

if [[ "${1:-}" == "--serve" ]]; then
  SERVE=1
else
  SERVE=0
fi

VENV_DIR=".venv/docs"

if [[ ! -d "${VENV_DIR}" ]]; then
  echo "[docs] Creating uv virtualenv at ${VENV_DIR}"
  uv venv "${VENV_DIR}"
fi

source "${VENV_DIR}/bin/activate"

CARGO_TOML="Cargo.toml"
ADDED_CDYLIB=0

cleanup() {
  if [[ "${ADDED_CDYLIB}" == "1" ]]; then
    echo "[docs] Restoring crate-type without cdylib"
    sed -i 's/crate-type = \["rlib", "cdylib"\]/crate-type = ["rlib"]/g' "${CARGO_TOML}"
  fi
}
trap cleanup EXIT

if grep -Fq 'crate-type = ["rlib"]' "${CARGO_TOML}"; then
  echo "[docs] Temporarily adding cdylib crate-type for doc build"
  sed -i 's/crate-type = \["rlib"\]/crate-type = ["rlib", "cdylib"]/g' "${CARGO_TOML}"
  ADDED_CDYLIB=1
fi

PYTHON="${VENV_DIR}/bin/python"
MATURIN="${VENV_DIR}/bin/maturin"
PDOC="${VENV_DIR}/bin/pdoc"
STUBGEN="${VENV_DIR}/bin/pyo3-stubgen"

echo "[docs] Installing tools (maturin, pdoc, pyo3-stubgen) into ${VENV_DIR}"
uv pip install --upgrade maturin pdoc pyo3-stubgen >/dev/null

echo "[docs] Building extension with maturin (develop mode)"
"${MATURIN}" develop --release

echo "[docs] Generating type stubs via pyo3-stubgen"
rm -rf stubs
"${STUBGEN}" bedder stubs

echo "[docs] Generating HTML docs via pdoc"
rm -rf docs
if "${PDOC}" bedder --output-dir docs; then
  echo "[docs] Built docs from live module"
else
  echo "[docs] Falling back to building docs from stubs"
  "${PDOC}" stubs/bedder/__init__.pyi --output-dir docs
fi

if [[ "$SERVE" == "1" ]]; then
  echo "[docs] Serving docs at http://127.0.0.1:8080"
  exec "${PDOC}" bedder --http 127.0.0.1:8080 || "${PDOC}" stubs/bedder/__init__.pyi --http 127.0.0.1:8080
fi

echo "[docs] Done. Output in ./docs"
