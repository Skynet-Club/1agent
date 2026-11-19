value=$(sed -n 's/^editor=//p' MEMORY.md | tail -n 1)
printf '%s\n' "$value" > answer.txt
