#!/usr/bin/env bash
# clear-zone-identifier.sh
# 专门用于 WSL：清理通过 Windows 文件管理器复制到 WSL 文件系统时
# 产生的 "*:Zone.Identifier" 附加文件。
#
# 用法:
#   ./clear-zone-identifier.sh [-n] [-q] [路径]
#
# 选项:
#   -n, --dry-run   仅显示，不删除
#   -q, --quiet     安静模式，只输出统计
#   -h, --help      帮助
#
# 路径默认为当前目录，递归处理。

set -uo pipefail

DRY_RUN=false
QUIET=false
ROOT=""

usage() {
    sed -n '2,15p' "$0" | sed 's/^# \{0,1\}//'
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        -n|--dry-run) DRY_RUN=true; shift ;;
        -q|--quiet)   QUIET=true; shift ;;
        -h|--help)    usage ;;
        --)           shift; break ;;
        -*)           echo "未知选项: $1" >&2; exit 1 ;;
        *)            ROOT="$1"; shift ;;
    esac
done

[[ -z "$ROOT" ]] && ROOT="$(pwd)"
[[ -d "$ROOT" ]] || { echo "错误: 目录不存在: $ROOT" >&2; exit 1; }
ROOT="$(cd "$ROOT" && pwd)"

# 匹配 "*:Zone.Identifier" 及其变体 "*:Zone.Identifier:$DATA"
# 注意：只看 basename 是否以 ":Zone.Identifier" 结尾
is_zone_file() {
    local base="${1##*/}"
    [[ "$base" == *:Zone.Identifier || "$base" == *:Zone.Identifier:\$DATA ]]
}

cleared=0
failed=0

# 用 -print0 + while read 处理任意文件名（包括空格、换行、冒号）
while IFS= read -r -d '' path; do
    if ! is_zone_file "$path"; then
        continue
    fi

    if $DRY_RUN; then
        $QUIET || echo "[dry-run] $path"
        ((cleared++))
        continue
    fi

    if rm -f -- "$path" 2>/dev/null; then
        $QUIET || echo "已删除: $path"
        ((cleared++))
    else
        echo "删除失败（可能需要 sudo）: $path" >&2
        ((failed++))
    fi
done < <(find "$ROOT" -type f \( \
            -name '*:Zone.Identifier' -o \
            -name '*:Zone.Identifier:$DATA' \
         \) -print0 2>/dev/null)

echo "完成。目录: $ROOT"
echo "  已清理: $cleared"
echo "  失败:   $failed"

(( failed == 0 ))
