#!/usr/bin/env bash
set -euo pipefail

# Local test runner to quickly run CI validations before push
echo "🧪 Running local Pylos validation suite..."
echo "=========================================="

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track results
FAILED=0

run_check() {
	local name=$1
	local cmd=$2
	echo ""
	echo "👉 Running: $name..."
	echo "------------------------------------------"
	if eval "$cmd"; then
		echo -e "${GREEN}✅ $name passed${NC}"
	else
		echo -e "${RED}❌ $name failed${NC}"
		FAILED=$((FAILED + 1))
	fi
}

# 1. Format check
run_check "Formatting Check" "cargo fmt --all -- --check"

# 2. Lint check (Clippy)
run_check "Clippy Lints" "cargo clippy --workspace --all-targets --all-features -- -D warnings"

# 3. Unit & Integration Tests
run_check "Unit & Integration Tests" "cargo test --workspace"

# 4. Security Audit (Optional check)
if command -v cargo-audit &>/dev/null; then
	run_check "Security Audit" "cargo audit"
else
	echo ""
	echo -e "${YELLOW}⚠️  cargo-audit not installed. Skip security check.${NC}"
fi

# Summary
echo ""
echo "=========================================="
if [ "$FAILED" -eq 0 ]; then
	echo -e "${GREEN}🎉 All local checks passed successfully! Ready to push.${NC}"
	exit 0
else
	echo -e "${RED}💥 $FAILED check(s) failed. Please review errors above before pushing.${NC}"
	exit 1
fi
