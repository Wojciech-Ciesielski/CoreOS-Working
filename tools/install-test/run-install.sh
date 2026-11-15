#!/bin/bash
# Test instalacji w QEMU: uruchamia agama-install.sh z okna Alt+F2, bez okna terminala. `script` daje
# pseudoterminal, więc sudo pamięta hasło dla całej sesji (jak w terminalu). Najpierw sprawdza edytor
# wiersza zsh w Cherry (wtyczki i narzędzia z obrazu); wynik idzie do dziennika (port szeregowy w teście).
D=$(dirname "$(readlink -f "$0")")
logger "shrimp-test: run-install $D"
if command -v zsh >/dev/null 2>&1 && [ -d "$HOME/.config/cherry/zsh" ]; then
  printf 'logger shrimp-test: zle-ok\nexit\n' \
    | ZDOTDIR="$HOME/.config/cherry/zsh" timeout 30 script -qec "zsh -i" /dev/null 2>&1 \
    | tr -d '\033' | tail -c 3000 | logger -t shrimp-test-zle
fi
exec script -qec "bash -c 'echo shrimp | sudo -S true; findmnt -l | sudo tee /dev/ttyS0 >/dev/null; echo \"script: $D/agama-install.sh\" | sudo tee /dev/ttyS0; bash \"$D/agama-install.sh\"'" /dev/null
