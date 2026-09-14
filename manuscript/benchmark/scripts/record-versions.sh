#!/usr/bin/env bash
set -euo pipefail

output=${1:-/dev/stdout}

{
  printf 'tool\tversion\tbinary_sha256\n'
  for tool in bedder bedtools bedtk bedops ailist coitrees-bed-intersect; do
    if [[ "$tool" == bedder ]]; then
      path=${BEDDER_BIN:-$(command -v bedder)}
    else
      path=$(command -v "$tool")
    fi
    version=$(
      case "$tool" in
        bedder) "$path" --version ;;
        bedtools) bedtools --version ;;
        bedtk) printf 'bedtk %s' "$(awk -F= '$1 == "bedtk" { print $2 }' /opt/bedder-benchmark/provenance/source-revisions.txt)" ;;
        bedops) bedops --version 2>&1 | awk '/version:/ && !found { print $2; found = 1 }' ;;
        ailist) ailist 2>&1 | head -n 1 || true ;;
        coitrees-bed-intersect) printf 'coitrees 0.4.0 example' ;;
      esac
    )
    sha=$(sha256sum "$path" | awk '{print $1}')
    printf '%s\t%s\t%s\n' "$tool" "$version" "$sha"
  done
  printf 'guest-kernel\t%s\t-\n' "$(uname -srmo)"
  printf 'guest-cpu\t%s\t-\n' "$(awk -F: '/model name/{sub(/^ /,"",$2); print $2; exit}' /proc/cpuinfo)"
} > "$output"
