#!/usr/bin/env bash
command -v curl >/dev/null || {
  echo "curl not found"
  exit 1
}
command -v python3 >/dev/null || {
  echo "python3 nof found"
  exit 1
}

source "$ROOT/lib/common.sh"
source "$ROOT/lib/http.sh"
source "$ROOT/lib/assertions.sh"
