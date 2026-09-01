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
    # 存量豁免：文件已超线但未继续变大（相比 HEAD）时只告警不拦截；
    # 新增文件或超线后继续变大的才拦截。
    head_lines=0
    if git cat-file -e "HEAD:$f" 2>/dev/null; then
        head_lines=$(git show "HEAD:$f" | wc -l)
    fi
    limit=$MAX_BUSINESS
    $is_test && limit=$MAX_TEST
    if (( lines > limit )); then
        if (( head_lines > 0 && lines <= head_lines )); then
            echo "WARN(存量): $f ${lines} 行（HEAD: ${head_lines}），未继续变大，暂不拦截"
        else
            echo "FAIL: $f ${lines} 行 > ${limit}（$($is_test && echo 测试 || echo 业务)文件上限）"
            failed=1
        fi
    elif ! $is_test && (( lines > WARN_BUSINESS )); then
        echo "WARN: $f ${lines} 行 > ${WARN_BUSINESS}（预警线，请审视职责）"
    fi
done
exit $failed
