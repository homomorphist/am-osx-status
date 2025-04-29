#!/usr/bin/env bash

# VSCode Workspaces don't currently let you interpolate environmental variables
# or expand shell aliases like "~", so it's impossible for us to reliably refer
# to the application data since we can't get the name of the user, and the
# location of this downloaded repository isn't reliable.
#
# We work around this by creating a symlink to the directory, so it still technically
# be accessed relative to this repository's location.
SCRIPT_LOCATION="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
ln -s "$HOME/Library/Application Support/am-osx-status/" "$SCRIPT_LOCATION/data"
