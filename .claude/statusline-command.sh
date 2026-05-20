#!/usr/bin/env bash

input=$(cat)

# Colors (ANSI)
CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
MAGENTA='\033[0;35m'
RED='\033[0;31m'
DIM='\033[2m'
RESET='\033[0m'
SEP="${DIM} | ${RESET}"

# --- Folder: last 2 path segments of cwd ---
cwd=$(echo "$input" | jq -r '.cwd // .workspace.current_dir // empty')
if [ -z "$cwd" ]; then
  cwd=$(pwd)
fi
folder=$(echo "$cwd" | awk -F'/' '{
  n=NF
  if (n >= 2) print $(n-1) "/" $n
  else print $n
}')

# --- Git branch ---
branch=$(git -C "$cwd" --no-optional-locks branch --show-current 2>/dev/null)

# --- Model display name ---
model=$(echo "$input" | jq -r '.model.display_name // empty')

# --- Token consumption ---
used_pct=$(echo "$input" | jq -r '.context_window.used_percentage // empty')
total_tokens=$(echo "$input" | jq -r '.context_window.total_input_tokens // empty')

# Format token count as compact k notation
if [ -n "$total_tokens" ] && [ "$total_tokens" -gt 0 ] 2>/dev/null; then
  tokens_k=$(awk "BEGIN { printf \"%.0fk\", $total_tokens / 1000 }")
else
  tokens_k=""
fi

# Pick color for usage percentage
if [ -n "$used_pct" ]; then
  pct_int=$(printf "%.0f" "$used_pct")
  if [ "$pct_int" -ge 80 ]; then
    PCT_COLOR="$RED"
  elif [ "$pct_int" -ge 50 ]; then
    PCT_COLOR="$YELLOW"
  else
    PCT_COLOR="$GREEN"
  fi
  if [ -n "$tokens_k" ]; then
    context_str="${PCT_COLOR}${pct_int}%${RESET}${DIM} (${tokens_k})${RESET}"
  else
    context_str="${PCT_COLOR}${pct_int}%${RESET}"
  fi
else
  context_str=""
fi

# --- Assemble status line ---
line="${CYAN}${folder}${RESET}"

if [ -n "$branch" ]; then
  line="${line}${SEP}${GREEN}${branch}${RESET}"
fi

if [ -n "$model" ]; then
  line="${line}${SEP}${MAGENTA}${model}${RESET}"
fi

if [ -n "$context_str" ]; then
  line="${line}${SEP}${context_str}"
fi

printf "${line}\n"
