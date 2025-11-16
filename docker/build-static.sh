#!/usr/bin/env bash
set -euo pipefail

find_tool() {
  for candidate in "$@"; do
    if [ -n "${candidate}" ] && command -v "${candidate}" >/dev/null 2>&1; then
      printf '%s' "${candidate}"
      return 0
    fi
  done
  return 1
}

resolve_python_root() {
  for candidate in "$@"; do
    if [ -n "${candidate}" ] && [ -d "${candidate}/install" ]; then
      printf '%s' "${candidate}"
      return 0
    fi
  done
  return 1
}

WORKDIR="${WORKDIR_PATH:-$PWD}"
if [ ! -f "${WORKDIR}/Cargo.toml" ]; then
  echo "error: ${WORKDIR} does not look like the bedder checkout (missing Cargo.toml)" >&2
  exit 1
fi

cd "${WORKDIR}"

TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
TARGET_ENV_VAR="$(printf '%s' "${TARGET_TRIPLE}" | tr '[:lower:]-' '[:upper:]_')"
TOOLS_WRAPPER_DIR="${WORKDIR}/.build-tools"
mkdir -p "${TOOLS_WRAPPER_DIR}"
export PATH="${TOOLS_WRAPPER_DIR}:${PATH}"

PYTHON_ROOT="$(resolve_python_root "${PYTHON_ROOT_OVERRIDE:-}" "${PYTHON_ROOT:-}" "/opt/python/python" "/opt/python-gnu/python" || true)"
if [ -z "${PYTHON_ROOT}" ]; then
  echo "error: unable to find Python root (expected install under /opt/python)" >&2
  exit 1
fi
PYTHON_INSTALL="${PYTHON_EMBED_HOME_OVERRIDE:-${PYTHON_EMBED_HOME:-${PYTHON_ROOT}/install}}"
PY_BUILD_LIB="${PYTHON_BUILD_LIB_OVERRIDE:-${PYTHON_BUILD_LIB:-${PYTHON_ROOT}/build/lib}}"
PYTHON_LIB_DIR="${PYTHON_LIB_DIR_OVERRIDE:-${PYO3_LIB_DIR:-${PYTHON_INSTALL}/lib}}"
PYTHON_BIN="${PYTHON_BIN_OVERRIDE:-${PYO3_PYTHON:-${PYTHON_INSTALL}/bin/python3}}"
PYTHON_INCLUDE_DIR="${PYTHON_INCLUDE_DIR_OVERRIDE:-${PYO3_INCLUDE_DIR:-${PYTHON_INSTALL}/include}}"
PYTHON_LIB_NAME="${PYO3_LIB_NAME:-python3.13}"

: "${PYO3_CONFIG_FILE:=$PWD/pyo3-config.txt}"
cat <<EOF > "${PYO3_CONFIG_FILE}"
implementation=CPython
version=3.13
shared=false
abi3=false
lib_name=${PYTHON_LIB_NAME}
lib_dir=${PYTHON_LIB_DIR}
executable=${PYTHON_BIN}
EOF

PYTHON_EXTRA_LIB_TOKENS="$(
  "${PYTHON_BIN}" <<'PY'
import shlex
import sysconfig

parts = []
for key in ("LOCALMODLIBS", "LIBS", "SYSLIBS"):
    value = sysconfig.get_config_var(key)
    if value:
        parts.extend(shlex.split(value))

print("\n".join(parts))
PY
)"

if [ -n "${PYTHON_EXTRA_LIB_TOKENS}" ]; then
  printf 'extra_build_script_line=cargo:rustc-link-search=native=%s\n' "${PY_BUILD_LIB}" >> "${PYO3_CONFIG_FILE}"
  while IFS= read -r token; do
    case "${token}" in
      -l*)
        lib="${token#-l}"
        if [ -f "${PY_BUILD_LIB}/lib${lib}.a" ] || [ -f "${PYTHON_LIB_DIR}/lib${lib}.a" ]; then
          printf 'extra_build_script_line=cargo:rustc-link-lib=static=%s\n' "${lib}" >> "${PYO3_CONFIG_FILE}"
        fi
        ;;
      -L*)
        printf 'extra_build_script_line=cargo:rustc-link-search=native=%s\n' "${token#-L}" >> "${PYO3_CONFIG_FILE}"
        ;;
    esac
  done <<< "${PYTHON_EXTRA_LIB_TOKENS}"
fi

export PYO3_CONFIG_FILE
export PYTHON_EMBED_HOME="${PYTHON_INSTALL}"
export PYTHON_BUILD_LIB="${PY_BUILD_LIB}"
export PYO3_LIB_DIR="${PYTHON_LIB_DIR}"
export PYO3_INCLUDE_DIR="${PYTHON_INCLUDE_DIR}"
export PYO3_PYTHON="${PYTHON_BIN}"
export PYO3_LIB_NAME="${PYTHON_LIB_NAME}"
export PYO3_STATIC=1
export PYTHON_SYS_EXECUTABLE="${PYTHON_BIN}"
export OPENSSL_DIR="${OPENSSL_DIR:-${PYTHON_INSTALL}}"
export OPENSSL_LIB_DIR="${OPENSSL_LIB_DIR:-${PY_BUILD_LIB}}"
export OPENSSL_INCLUDE_DIR="${OPENSSL_INCLUDE_DIR:-${PYTHON_INSTALL}/include}"
export OPENSSL_STATIC=1
export LIBZ_SYS_STATIC=1
export LIBZ_STATIC=1
export ZLIB_STATIC=1
export BZIP2_STATIC=1
export LZMA_API_STATIC=1
export PKG_CONFIG_ALL_STATIC=1

export BINDGEN_USE_CLI=1
CLANG_CANDIDATE="${CLANG_PATH:-}"
if [ -z "${CLANG_CANDIDATE}" ] || ! command -v "${CLANG_CANDIDATE}" >/dev/null 2>&1; then
  CLANG_CANDIDATE="$(find_tool clang-20 clang cc || true)"
fi
if [ -z "${CLANG_CANDIDATE}" ]; then
  echo "error: unable to find clang compiler" >&2
  exit 1
fi
export CLANG_PATH="${CLANG_CANDIDATE}"
LINKER="${CLANG_PATH}"

LLD_BIN="${LLD_BIN:-}"
if [ -z "${LLD_BIN}" ] || ! command -v "${LLD_BIN}" >/dev/null 2>&1; then
  LLD_BIN="$(find_tool lld lld-20 ld.lld || true)"
fi
if [ -z "${LLD_BIN}" ]; then
  echo "error: unable to find lld linker" >&2
  exit 1
fi
if ! command -v lld >/dev/null 2>&1; then
  cat <<EOF > "${TOOLS_WRAPPER_DIR}/lld"
#!/usr/bin/env bash
exec "${LLD_BIN}" "\$@"
EOF
  chmod +x "${TOOLS_WRAPPER_DIR}/lld"
fi
LLD_FUSE_ARG="lld"

CXX_BIN="${CXX_BIN:-}"
if [ -z "${CXX_BIN}" ] || ! command -v "${CXX_BIN}" >/dev/null 2>&1; then
  CXX_BIN="$(find_tool clang++-20 clang++ || true)"
fi
if [ -z "${CXX_BIN}" ]; then
  echo "error: unable to find clang++" >&2
  exit 1
fi

AR_BIN="${AR_BIN:-}"
if [ -z "${AR_BIN}" ] || ! command -v "${AR_BIN}" >/dev/null 2>&1; then
  AR_BIN="$(find_tool llvm-ar-20 llvm-ar ar || true)"
fi
if [ -z "${AR_BIN}" ]; then
  echo "error: unable to find llvm-ar/ar" >&2
  exit 1
fi

RANLIB_BIN="${RANLIB_BIN:-}"
if [ -z "${RANLIB_BIN}" ] || ! command -v "${RANLIB_BIN}" >/dev/null 2>&1; then
  RANLIB_BIN="$(find_tool llvm-ranlib-20 llvm-ranlib ranlib || true)"
fi
if [ -z "${RANLIB_BIN}" ]; then
  echo "error: unable to find llvm-ranlib/ranlib" >&2
  exit 1
fi

export "CC_${TARGET_ENV_VAR}=${LINKER}"
export "CXX_${TARGET_ENV_VAR}=${CXX_BIN}"
export "AR_${TARGET_ENV_VAR}=${AR_BIN}"
export "RANLIB_${TARGET_ENV_VAR}=${RANLIB_BIN}"
export "CARGO_TARGET_${TARGET_ENV_VAR}_LINKER=${LINKER}"
RUSTFLAGS_VALUE="-C target-feature=+crt-static -C linker=${LINKER} -C link-arg=-fuse-ld=${LLD_FUSE_ARG} -C link-arg=-static -C relocation-model=static"
eval "export CARGO_TARGET_${TARGET_ENV_VAR}_RUSTFLAGS=\"${RUSTFLAGS_VALUE} \${CARGO_TARGET_${TARGET_ENV_VAR}_RUSTFLAGS-}\""

CARGO_BIN="${CARGO_BIN:-/usr/local/cargo/bin/cargo}"

echo "Building bedder (${TARGET_TRIPLE}) with static linking..."
"${CARGO_BIN}" build --release --target "${TARGET_TRIPLE}" "$@"

BIN="target/${TARGET_TRIPLE}/release/bedder"
OUT="dist/bedder-${TARGET_TRIPLE}-static"
mkdir -p dist
cp "${BIN}" "${OUT}"
if command -v llvm-strip-20 >/dev/null 2>&1; then
  llvm-strip-20 "${OUT}" || true
else
  strip "${OUT}" || true
fi

ldd_output="$(ldd "${OUT}" 2>&1 || true)"
echo "${ldd_output}"
if ! grep -q "not a dynamic executable" <<<"${ldd_output}"; then
  echo "error: ${OUT} is not fully static" >&2
  exit 1
fi

if [ -f /workspace/a.bed ] && [ -f /workspace/b.bed ] && [ -f /workspace/g.genome ]; then
  OUT_FILE="${WORKDIR}/dist/intersect-check.txt"
  "${OUT}" intersect -a /workspace/a.bed -b /workspace/b.bed -g /workspace/g.genome >"${OUT_FILE}"
  if [ ! -s "${OUT_FILE}" ]; then
    echo "error: intersect smoke test produced no output" >&2
    exit 1
  fi
  echo "wrote smoke test output to ${OUT_FILE}"
else
  echo "skip smoke test: /workspace/{a.bed,b.bed,g.genome} missing"
fi

echo "Static binary available at ${OUT}"
