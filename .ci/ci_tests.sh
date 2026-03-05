#!/usr/bin/env bash
set -euo pipefail

# ────────────────────────────────────────────────
# CI test selector via .ci_tests.env
#   TEST_WITH=miri     -> cargo miri test
#   TEST_WITH=cargo    -> cargo test
#   TEST_WITH=nextest  -> cargo nextest run
#   TEST_WITH=nothing  -> skip tests
#   (missing file/key defaults to cargo)
#
# Exit code propagates to caller:
# - test failures return non-zero and this script exits non-zero
# - unknown TEST_WITH exits 1
# ────────────────────────────────────────────────

repo_root="$(git rev-parse --show-toplevel)"
ENV_FILE="$repo_root/.ci/ci_tests.env"
TEST_WITH=

if [[ -f "$ENV_FILE" ]]; then
  val="$(grep -E '^[[:space:]]*TEST_WITH=' "$ENV_FILE" |
    tail -n 1 |
    cut -d= -f2- |
    tr -d '[:space:]\r')"
  [[ -n "$val" ]] && TEST_WITH="$val"
fi

case "$TEST_WITH" in
miri)
  echo "▶ Running: cargo miri test"
  cargo miri test
  ;;
cargo)
  echo "▶ Running: cargo test"
  cargo test
  ;;
nextest)
  echo "▶ Running: cargo nextest run"
  cargo nextest run
  ;;
nothing)
  echo "▶ Skipping tests (TEST_WITH=nothing)"
  exit 0
  ;;
*)
  echo "❌ ERROR: Unknown environemt, TEST_WITH='$TEST_WITH' (expected: miri|cargo|nextest|nothing)" >&2
  exit 1
  ;;
esac
