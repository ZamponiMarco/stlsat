#!/bin/bash
set -eu

INPUT=$1
shift

EXTRA_ARGS=("$@")
STLSAT="../target/release/stlsat"

parallel --lb --halt now,success=1 ::: \
  "$STLSAT --engine tableau ${EXTRA_ARGS[*]} $INPUT" \
  "$STLSAT --engine fol ${EXTRA_ARGS[*]} $INPUT" \
  "$STLSAT --engine smt ${EXTRA_ARGS[*]} $INPUT"
