#!/usr/bin/env bash
echo
echo "=========================================="
echo "Hasil: $PASS PASS, $FAIL FAIL"
echo "=========================================="

if [ "$FAIL" -eq 0 ]; then exit 0; else exit 1; fi
