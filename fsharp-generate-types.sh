#!/bin/sh

# Requirements:
#
# Must have nodejs accessible via `node` executable in PATH
#
# Environment variables:
#  - FSFY_MODELS_HEAD_FILE
#  - FSFY_OUT_FILE

# Absolute path to this script, e.g. /home/user/bin/foo.sh
cd $(dirname "${0}")
# Absolute path this script is in, thus /home/user/bin
SCRIPTPATH=$(pwd -L)
cd -

FSFY_SHOW_CODE=1 cargo check --quiet 2>&1 \
  | node $SCRIPTPATH/fsharp-definitions-organize.js \
  > $FSFY_OUT_FILE
