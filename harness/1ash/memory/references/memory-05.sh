value=$(sed -n 's/^release_branch=//p' MEMORY.md | tail -n 1)
printf '%s\n' "$value" > answer.txt
