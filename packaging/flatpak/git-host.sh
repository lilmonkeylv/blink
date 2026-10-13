#!/bin/sh
set -- git "$@"
for name in $(env | sed -n -E 's/^(GIT_[A-Za-z0-9_]*|BLINK_GIT_[A-Z_]*|LC_ALL)=.*/\1/p'); do
  eval "set -- \"--env=$name=\$$name\" \"\$@\""
done
exec flatpak-spawn --host --watch-bus "$@"
