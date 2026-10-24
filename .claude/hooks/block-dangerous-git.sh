#!/usr/bin/env bash
# Blocks destructive git commands from being executed by AI agents.
# Receives tool input as JSON on stdin; exits 2 to block, 0 to allow.

INPUT=$(cat)
COMMAND=$(echo "$INPUT" | python3 -c "import sys,json; print(json.load(sys.stdin).get('command',''))" 2>/dev/null)

if [ -z "$COMMAND" ]; then
  exit 0
fi

DANGEROUS_PATTERNS=(
  # Discard all local changes
  'git[[:space:]]+reset[[:space:]]+--hard'
  # Force push (any flag order)
  'git[[:space:]]+push[[:space:]]+(--force|--force-with-lease)'
  'git[[:space:]]+push[[:space:]]+-[a-zA-Z]*f'
  # Remove untracked files/dirs (catches -f -fd -fx -fxd -dfx etc.)
  'git[[:space:]]+clean[[:space:]]+-[a-zA-Z]*[fdx]'
  # Force-delete branch
  'git[[:space:]]+branch[[:space:]]+-D'
  # Discard all working tree changes
  'git[[:space:]]+checkout[[:space:]]+(--[[:space:]]+)?\.'
  # Restore all files (blocks `. ` or `-- .` but not specific files)
  'git[[:space:]]+restore[[:space:]]+(--[a-z-]+[[:space:]]+)*\.'
  # Destroy stash entries
  'git[[:space:]]+stash[[:space:]]+(drop|clear)'
  # Rewrite / delete history
  'git[[:space:]]+reflog[[:space:]]+(delete|expire)'
  'git[[:space:]]+filter-branch'
  'git[[:space:]]+filter-repo'
  'git[[:space:]]+update-ref[[:space:]]+-d'
  # Delete tags and remotes
  'git[[:space:]]+tag[[:space:]]+-d'
  'git[[:space:]]+remote[[:space:]]+remove'
  # Aggressive garbage collect
  'git[[:space:]]+gc[[:space:]]+--prune'
)

for PATTERN in "${DANGEROUS_PATTERNS[@]}"; do
  if echo "$COMMAND" | grep -qE "$PATTERN"; then
    echo ""
    echo "🚫 BLOCKED: Dangerous git command detected."
    echo "   Command : $COMMAND"
    echo "   Reason  : matches guardrail pattern '$PATTERN'"
    echo ""
    echo "If you need to run this command, execute it manually in your terminal."
    exit 2
  fi
done

exit 0
