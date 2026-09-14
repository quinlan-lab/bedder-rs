#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
benchmark_dir=$(cd "$script_dir/.." && pwd)
source "$benchmark_dir/config.env"

runtime_root=${BEDDER_CMP_ROOT:-/media/brentp/elements/bedder-cmp}
image_archive="$runtime_root/bedder-benchmark-image.tar"

mkdir -p "$runtime_root"

docker build \
  --file "$benchmark_dir/docker/Dockerfile" \
  --tag "$BENCH_IMAGE_TAG" \
  --build-arg "BEDDER_REF=$BEDDER_REF" \
  --build-arg "BEDTOOLS_REF=$BEDTOOLS_REF" \
  --build-arg "BEDTK_REF=$BEDTK_REF" \
  --build-arg "AILIST_REF=$AILIST_REF" \
  --build-arg "COITREES_REF=$COITREES_REF" \
  --build-arg "BEDOPS_VERSION=$BEDOPS_VERSION" \
  --build-arg "RUST_VERSION=$RUST_VERSION" \
  "$benchmark_dir/docker"

tmp_archive="$image_archive.tmp"
docker save "$BENCH_IMAGE_TAG" --output "$tmp_archive"
mv "$tmp_archive" "$image_archive"
sha256sum "$image_archive" > "$image_archive.sha256"

printf 'created %s\n' "$image_archive"
