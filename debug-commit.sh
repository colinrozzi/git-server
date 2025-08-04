#!/bin/bash
set -e

# This script will help debug the exact Git commit format
cd /Users/colinrozzi/work/actor-registry/git-server/tests

# Create the same commit locally to inspect its format
mkdir debug-commit-test
cd debug-commit-test

# Set the same git config as your test
git config user.name "colin"
git config user.email "colinrozzi@gmail.com"

# Create the exact same commit
git init
echo "test" > test.txt
git add .

# Set the commit timestamp to match yours (1754333075 = Mon Aug  4 14:44:35 2025 -0400)
GIT_COMMITTER_DATE="1754333075 -0400" GIT_AUTHOR_DATE="1754333075 -0400" git commit -m "Initial commit"

# Get the commit hash
COMMIT_HASH=$(git rev-parse HEAD)
echo "Created commit: $COMMIT_HASH"

# Show the raw commit object
echo "Raw commit object:"
git cat-file -p $COMMIT_HASH
echo ""

echo "Raw commit bytes:"
git cat-file commit $COMMIT_HASH | xxd

echo ""
echo "Commit size:"
git cat-file -s $COMMIT_HASH

# Clean up
cd ..
rm -rf debug-commit-test
