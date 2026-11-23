#!/bin/sh
# Run from anywhere in the checkout. Stop with Ctrl-C.
set -eu
repo=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
server=${LLAMA_SERVER:-/Volumes/FastSSD/inkling-pi-test/runtime/build/bin/llama-server}
base=${INKLING_BASE:-/Volumes/FastSSD/inkling-pi-test/models/Inkling-2-Mini-IQ4_NL.gguf}
adapter=${INKLING_ADAPTER:-$repo/runs/inkling-tuning/001-river/selected-adapter/poker.gguf}
exec "$server" -m "$base" --lora "$adapter" --alias inkling-poker-001 --host 127.0.0.1 --port 9933 --ctx-size 8192 --parallel 1 --gpu-layers all --jinja --reasoning off --chat-template-kwargs '{"reasoning_effort":"none"}'
