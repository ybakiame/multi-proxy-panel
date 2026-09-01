#!/usr/bin/env bash
# File-size guard per .agents/rules/code-organization.md
# Usage: check-file-size.sh [file ...]  (default: all tracked source files)
# Limits: business source ≤ 500 lines (hard fail), tests ≤ 1000 lines (hard fail),
# warning zone starts at 400 lines (warn only, non-blocking).
set -euo pipefail

WARN_BUSINESS=400
MAX_BUSINESS=500
MAX_TEST=1000

if [[ $# -gt 0 ]]; then
    files=("$@")
else
    mapfile -t files < <(git ls-files)
fi

failed=0
for f in "${files[@]}"; do
    [[ -f "$f" ]] || continue
    case "$f" in
        *.rs|*.ts|*.tsx|*.go|*.kt) ;;
        *) continue ;;
    esac
    # skip generated / vendored / dependency dirs
    case "$f" in
        */target/*|*/node_modules/*|*/gen/*|*/dist/*|*/.heroui-docs/*|.reference/*) continue ;;
    esac
    lines=$(wc -l < "$f")
    is_test=false
    case "$f" in
        */tests/*|*tests.rs|*.test.ts|*.test.tsx|*/integration_tests.rs) is_test=true ;;
    esac
    if $is_test; then
        if (( lines > MAX_TEST )); then
            echo "FAIL(test): $f ${lines} 行 > ${MAX_TEST}"
            failed=1
        fi
    else
        if (( lines > MAX_BUSINESS )); then
            echo "FAIL: $f ${lines} 行 > ${MAX_BUSINESS}（业务文件上限）"
            failed=1
        elif (( lines > WARN_BUSINESS )); then
            echo "WARN: $f ${lines} 行 > ${WARN_BUSINESS}（预警线，请审视职责）"
        fi
    fi
done
exit $failed
