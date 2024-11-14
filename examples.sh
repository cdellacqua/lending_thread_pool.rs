#!/bin/sh

for EXAMPLE in examples/*.rs; do
	EXAMPLE="$(basename "$EXAMPLE" | rev | cut -c4- | rev)"
	echo "[EXAMPLE($EXAMPLE)] running..."
	cargo run --example "$EXAMPLE"
	echo "[EXAMPLE($EXAMPLE)] done"
done
