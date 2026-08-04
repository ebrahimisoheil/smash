#!/usr/bin/env bash
set -euo pipefail

ARTIFACT_DIR="${1:?usage: $0 ARTIFACT_DIR}"
for required in SHA256SUMS server.json ENVIRONMENT_VARIABLES PACKAGE_CONTENTS IMAGE_DIGESTS BUILD_MANIFEST NATIVE_TARGET RELEASE_TAG; do
  test -f "$ARTIFACT_DIR/$required" || { echo "missing release file: $required" >&2; exit 1; }
done
if command -v sha256sum >/dev/null 2>&1; then
  (cd "$ARTIFACT_DIR" && sha256sum --check SHA256SUMS)
else
  (cd "$ARTIFACT_DIR" && shasum -a 256 -c SHA256SUMS)
fi
! rg -n 'BEGIN (RSA|OPENSSH|EC|PRIVATE) KEY|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{20,}' "$ARTIFACT_DIR"
! rg -n '=' "$ARTIFACT_DIR/ENVIRONMENT_VARIABLES"
rg -n '^[A-Z][A-Z0-9_]*$' "$ARTIFACT_DIR/ENVIRONMENT_VARIABLES" >/dev/null
rg -n '^v[0-9]+\.[0-9]+\.[0-9]+$' "$ARTIFACT_DIR/RELEASE_TAG" >/dev/null
rg -n '"name": "io\.github\.ebrahimisoheil/engrave"|"version":' "$ARTIFACT_DIR/server.json" >/dev/null
python3 "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/verify-registry-metadata.py" "$ARTIFACT_DIR/server.json" "$ARTIFACT_DIR" >/dev/null
version="$(sed -n 's/^[[:space:]]*"version":[[:space:]]*"\([^"]*\)"[,]*[[:space:]]*$/\1/p' "$ARTIFACT_DIR/server.json" | head -n 1)"
test -n "$version" || { echo "missing version in server.json" >&2; exit 1; }
test "$(tr -d '\r\n' < "$ARTIFACT_DIR/RELEASE_TAG")" = "v$version" || {
  echo "RELEASE_TAG does not match server.json version" >&2
  exit 1
}
rg -n "^version=$version$" "$ARTIFACT_DIR/BUILD_MANIFEST" >/dev/null || {
  echo "BUILD_MANIFEST version mismatch" >&2
  exit 1
}
rg -n "^release_tag=v$version$" "$ARTIFACT_DIR/BUILD_MANIFEST" >/dev/null || {
  echo "BUILD_MANIFEST tag mismatch" >&2
  exit 1
}
rg -n '^source_commit=[0-9a-f]{40}$' "$ARTIFACT_DIR/BUILD_MANIFEST" >/dev/null || {
  echo "BUILD_MANIFEST source commit is missing or malformed" >&2
  exit 1
}
rg -n '^native_target=[A-Za-z0-9._-]+$' "$ARTIFACT_DIR/BUILD_MANIFEST" >/dev/null || {
  echo "BUILD_MANIFEST native target is missing or malformed" >&2
  exit 1
}
rg -n '^release_targets=.+$' "$ARTIFACT_DIR/BUILD_MANIFEST" >/dev/null || {
  echo "BUILD_MANIFEST release targets are missing" >&2
  exit 1
}
rg -n '^rustc_version=rustc .+$' "$ARTIFACT_DIR/BUILD_MANIFEST" >/dev/null || {
  echo "BUILD_MANIFEST compiler version is missing" >&2
  exit 1
}
rg -n "^engrave-v2:$version sha256:[0-9a-f]{64}$" "$ARTIFACT_DIR/IMAGE_DIGESTS" >/dev/null || {
  echo "missing or malformed API image digest" >&2
  exit 1
}
rg -n "^engrave-v2-web:$version sha256:[0-9a-f]{64}$" "$ARTIFACT_DIR/IMAGE_DIGESTS" >/dev/null || {
  echo "missing or malformed web image digest" >&2
  exit 1
}
expected_manifest_images="$(sed 's/^/image_/' "$ARTIFACT_DIR/IMAGE_DIGESTS")"
actual_manifest_images="$(rg '^image_' "$ARTIFACT_DIR/BUILD_MANIFEST")"
test "$expected_manifest_images" = "$actual_manifest_images" || {
  echo "BUILD_MANIFEST image identities do not match IMAGE_DIGESTS" >&2
  exit 1
}
bundle="$ARTIFACT_DIR/engrave-community-$version.tar.gz"
test -f "$bundle" || { echo "missing Community Edition bundle: $bundle" >&2; exit 1; }
required_bundle_files=(
  compose.yaml
  .env.example
  docs/community-edition.md
  docs/release-review.md
  SECURITY.md
  CONTRIBUTING.md
  scripts/backup-local.sh
  scripts/restore-local.sh
  scripts/bootstrap-local.sh
  scripts/verify-release.sh
  scripts/verify-registry-metadata.py
  scripts/validate-registry-official.py
)
for required_entry in "${required_bundle_files[@]}"; do
  tar -tzf "$bundle" | rg -Fx "$required_entry" >/dev/null || {
    echo "missing required bundle entry: $required_entry" >&2
    exit 1
  }
done
for required_prefix in migrations/ plugin/; do
  tar -tzf "$bundle" | rg -F "$required_prefix" >/dev/null || {
    echo "missing required bundle directory: $required_prefix" >&2
    exit 1
  }
done
! tar -tzf "$bundle" | rg '(^|/)\.\.?(/|$)|^/'
if tar -xOf "$bundle" compose.yaml .env.example docs/community-edition.md docs/release-review.md SECURITY.md CONTRIBUTING.md \
  scripts/backup-local.sh scripts/restore-local.sh scripts/bootstrap-local.sh scripts/verify-release.sh \
  scripts/verify-registry-metadata.py scripts/validate-registry-official.py \
  | rg -n 'BEGIN (RSA|OPENSSH|EC|PRIVATE) KEY|AKIA[0-9A-Z]{16}|sk-[A-Za-z0-9]{20,}' >/dev/null; then
  echo "secret-like material found in Community Edition bundle" >&2
  exit 1
fi
actual_contents="$(find "$ARTIFACT_DIR" -maxdepth 1 -type f -exec basename {} \; | sort)"
declared_contents="$(sort "$ARTIFACT_DIR/PACKAGE_CONTENTS")"
if test "$actual_contents" != "$declared_contents"; then
  echo "PACKAGE_CONTENTS does not match the artifact directory" >&2
  diff -u <(printf '%s\n' "$declared_contents") <(printf '%s\n' "$actual_contents") >&2 || true
  exit 1
fi
native_target="$(tr -d '\n' < "$ARTIFACT_DIR/NATIVE_TARGET")"
manifest_native_target="$(sed -n 's/^native_target=//p' "$ARTIFACT_DIR/BUILD_MANIFEST")"
test "$native_target" = "$manifest_native_target" || {
  echo "NATIVE_TARGET does not match BUILD_MANIFEST" >&2
  exit 1
}
native_binary="$ARTIFACT_DIR/engrave-mcp-$version-$native_target"
test -x "$native_binary" || { echo "native MCP artifact is not executable: $native_binary" >&2; exit 1; }
initialize_response="$(printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize"}' | "$native_binary")"
printf '%s\n' "$initialize_response" | rg '"protocolVersion"' >/dev/null || {
  echo "native MCP stdio initialize failed" >&2
  exit 1
}
printf '%s\n' "$native_target" | rg '^[A-Za-z0-9._-]+$' >/dev/null
find "$ARTIFACT_DIR" -type f -print0 | xargs -0 file | rg -i 'text|json|executable' >/dev/null
echo "release artifact verification passed: $ARTIFACT_DIR"
