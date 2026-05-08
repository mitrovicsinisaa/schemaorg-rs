#!/bin/bash
# Build WASM binaries for both web (browser ESM) and Node.js targets.
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-pack
#   brew install binaryen  (for wasm-opt)
#
# Usage:
#   ./scripts/build-wasm.sh          # build both targets
#   ./scripts/build-wasm.sh web      # web only
#   ./scripts/build-wasm.sh node     # node only

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
OUT_DIR="$PROJECT_DIR/wasm/pkg"

TARGET="${1:-both}"

build_target() {
    local target="$1"
    local out="$OUT_DIR/$target"

    echo "═══════════════════════════════════════════════════"
    echo "  Building WASM: --target $target"
    echo "═══════════════════════════════════════════════════"

    wasm-pack build -t "$target" -d "$out" --out-name schemaorg_rs --no-opt "$PROJECT_DIR" -- --features wasm

    # Post-optimize with wasm-opt (with bulk-memory enabled for Rust output)
    local wasm_file="$out/schemaorg_rs_bg.wasm"
    if command -v wasm-opt &> /dev/null && [ -f "$wasm_file" ]; then
        local before_size
        before_size=$(wc -c < "$wasm_file" | tr -d ' ')
        wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext "$wasm_file" -o "$wasm_file"
        local after_size
        after_size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  wasm-opt: ${before_size} → ${after_size} bytes ($(( (before_size - after_size) * 100 / before_size ))% reduction)"
    fi

    # Size report
    echo ""
    echo "  Output: $out/"
    ls -lh "$wasm_file" 2>/dev/null || true
    echo ""
}

case "$TARGET" in
    web)
        build_target "web"
        ;;
    node|nodejs)
        build_target "nodejs"
        ;;
    both)
        build_target "web"
        build_target "nodejs"
        ;;
    *)
        echo "Usage: $0 [web|node|both]"
        exit 1
        ;;
esac

# Size gate
echo "═══════════════════════════════════════════════════"
echo "  Size Gate (target: <500KB)"
echo "═══════════════════════════════════════════════════"

for dir in "$OUT_DIR"/*/; do
    wasm_file="$dir/schemaorg_rs_bg.wasm"
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        size_kb=$((size / 1024))
        target_name=$(basename "$dir")
        if [ "$size_kb" -lt 500 ]; then
            echo "  ✅ $target_name: ${size_kb}KB"
        else
            echo "  ❌ $target_name: ${size_kb}KB (exceeds 500KB budget!)"
        fi
    fi
done
echo ""
