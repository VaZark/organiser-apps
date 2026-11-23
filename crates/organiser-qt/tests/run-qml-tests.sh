#!/bin/sh
# Qt Quick tests of the shared QML (needs Qt 6's qmltestrunner and Kirigami; not part of `cargo test`).
# The apps get `i18n` from KI18n; the test has none, so it runs on a copy that uses `qsTr`.
set -e
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
sed 's/i18n(/qsTr(/g' "$here/../qml/TagPicker.qml" > "$work/TagPicker.qml"
cp "$here"/tst_*.qml "$work/"
cd "$work"
QT_QPA_PLATFORM="${QT_QPA_PLATFORM:-offscreen}" \
    "${QMLTESTRUNNER:-/usr/lib/qt6/bin/qmltestrunner}" -import . -input .
