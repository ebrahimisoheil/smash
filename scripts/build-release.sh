#!/usr/bin/env bash
set -euo pipefail

# Build release artifacts only from an exact, clean git tag. This script does
# not push images, publish packages, or publish Registry metadata.
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
VERSION="${1:?usage: $0 VERSION [OUTPUT_DIR]}"
OUTPUT_DIR="${2:-dist/$VERSION}"
TAG="$(git describe --tags --exact-match 2>/dev/null || true)"
test "$TAG" = "v$VERSION" || { echo "release must be built at exact tag v$VERSION (found: ${TAG:-none})" >&2; exit 1; }
test -z "$(git status --porcelain)" || { echo "release tree is dirty" >&2; exit 1; }
mkdir -p "$OUTPUT_DIR"
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$@"; else shasum -a 256 "$@"; fi
}

cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
RELEASE_TARGETS="${RELEASE_TARGETS:-$HOST_TARGET}"
printf '%s\n' "$HOST_TARGET" > "$OUTPUT_DIR/NATIVE_TARGET"
printf '%s\n' "$TAG" > "$OUTPUT_DIR/RELEASE_TAG"
for target in $RELEASE_TARGETS; do
  cargo build --locked --release -p engrave-mcp --target "$target"
  binary="target/$target/release/mcp"
  test -x "$binary" || { echo "missing executable for target $target: $binary" >&2; exit 1; }
  install -m 0755 "$binary" "$OUTPUT_DIR/engrave-mcp-$VERSION-$target"
done
command -v npm >/dev/null 2>&1 || { echo "npm is required to verify the web release" >&2; exit 1; }
npm --prefix apps/web ci --ignore-scripts
npm --prefix apps/web audit --audit-level=high
npm --prefix apps/web run build
command -v docker >/dev/null 2>&1 || { echo "Docker is required to build versioned release images" >&2; exit 1; }
docker build --pull=false --tag "engrave-v2:$VERSION" .
docker build --pull=false --tag "engrave-v2-web:$VERSION" ./apps/web
if command -v docker >/dev/null 2>&1; then
  {
    docker image inspect "engrave-v2:$VERSION" --format 'engrave-v2:'"$VERSION"' {{.Id}}'
    docker image inspect "engrave-v2-web:$VERSION" --format 'engrave-v2-web:'"$VERSION"' {{.Id}}'
  } > "$OUTPUT_DIR/IMAGE_DIGESTS"
fi
{
  printf 'version=%s\n' "$VERSION"
  printf 'release_tag=%s\n' "$TAG"
  printf 'source_commit=%s\n' "$(git rev-parse HEAD)"
  printf 'native_target=%s\n' "$HOST_TARGET"
  printf 'release_targets=%s\n' "$RELEASE_TARGETS"
  printf 'rustc_version=%s\n' "$(rustc --version)"
  sed 's/^/image_/' "$OUTPUT_DIR/IMAGE_DIGESTS"
} > "$OUTPUT_DIR/BUILD_MANIFEST"
python3 scripts/generate-registry-metadata.py "$VERSION" "$OUTPUT_DIR" > "$OUTPUT_DIR/server.json"
python3 scripts/verify-registry-metadata.py "$OUTPUT_DIR/server.json" "$OUTPUT_DIR"
if test "${MCP_REGISTRY_VALIDATE_OFFICIAL:-0}" = "1"; then
  python3 scripts/validate-registry-official.py "$OUTPUT_DIR/server.json"
fi
sed -n 's/^\([A-Z][A-Z0-9_]*\)=.*/\1/p' .env.example | sort -u > "$OUTPUT_DIR/ENVIRONMENT_VARIABLES"
tar -czf "$OUTPUT_DIR/engrave-community-$VERSION.tar.gz" \
  compose.yaml .env.example docs/community-edition.md docs/release-review.md SECURITY.md CONTRIBUTING.md \
  scripts/backup-local.sh scripts/restore-local.sh scripts/verify-release.sh \
  scripts/bootstrap-local.sh \
  scripts/verify-registry-metadata.py scripts/validate-registry-official.py \
  migrations plugin
(cd "$OUTPUT_DIR" && sha256 BUILD_MANIFEST engrave-mcp-* engrave-community-*.tar.gz) > "$OUTPUT_DIR/SHA256SUMS"
touch "$OUTPUT_DIR/PACKAGE_CONTENTS"
find "$OUTPUT_DIR" -maxdepth 1 -type f -print \
  | sed "s#^$OUTPUT_DIR/##" | sort > "$OUTPUT_DIR/PACKAGE_CONTENTS"
printf '%s\n' "release artifacts written to $OUTPUT_DIR"
