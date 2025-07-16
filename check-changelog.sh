#!/bin/bash
version=$(grep '^version = ' Cargo.toml | cut -d'"' -f2)
if ! grep -q "## \[$version\]" CHANGELOG.md; then
  echo "Changelog does not contain version $version"
  exit 1
fi
