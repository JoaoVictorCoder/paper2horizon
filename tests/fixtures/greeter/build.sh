#!/bin/sh
# Builds greeter.jar, the test plugin, against the Bukkit stubs in ../stubs (only the shapes the
# plugin uses, under Bukkit's own names). Run it after changing the plugin; the jar is committed.
set -eu
cd "$(dirname "$0")"
OUT=$(mktemp -d)
javac --release 17 -d "$OUT/stubs" $(find ../stubs/src -name '*.java')
javac --release 17 -cp "$OUT/stubs" -d "$OUT/classes" $(find src -name '*.java')
cp resources/* "$OUT/classes/"
jar --create --file greeter.jar -C "$OUT/classes" .
rm -rf "$OUT"
echo greeter.jar
