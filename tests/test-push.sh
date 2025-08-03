#!/bin/bash
# This script initializes a Git repository, creates a file, commits it,

mkdir tmp-dir
cd tmp-dir

git init
echo "test" > test.txt
git add test.txt
git commit -m "Initial commit"

git remote add wasm http://localhost:8080
git push wasm master

cd ..
rm -rf tmp-dir
