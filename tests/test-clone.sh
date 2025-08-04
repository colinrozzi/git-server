#!/bin/bash
# This script initializes a Git repository, creates a file, commits it,

mkdir tmp-dir
cd tmp-dir

git init
echo "test" > test.txt
git add test.txt
git commit -m "Initial commit"

git remote add wasm http://localhost:8080
GIT_TRACE_PACKET=1 GIT_TRACE=1 GIT_RECEIVE_PACK_DEBUG=1 git push -u wasm main

cd ..

GIT_TRACE_PACKET=1 GIT_TRACE=1 GIT_RECEIVE_PACK_DEBUG=1 git clone http://localhost:8080 tmp-clone

rm -rf tmp-dir
rm -rf tmp-clone
