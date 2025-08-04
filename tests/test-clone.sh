#!/bin/bash
# This script initializes a Git repository, creates a file, commits it,

actor_id=$(theater start ../manifest.toml)
echo "Server started with actor ID: $actor_id"

echo "Giving the server time..."
# sleep for a beat
sleep 1

mkdir tmp-dir
cd tmp-dir

git init
echo "test" > test.txt
git add .
git commit -m "Initial commit"

git remote add wasm http://localhost:8080
git push -u wasm main

cd ..

GIT_TRACE_PACKET=1 GIT_TRACE_CURL=1 git clone http://localhost:8080 tmp-clone

echo "Contents of tmp-clone:"
ls -la tmp-clone
echo " ";

echo "Contents of tmp-dir:"
ls -la tmp-dir
echo " ";

rm -rf tmp-dir
rm -rf tmp-clone
theater stop $actor_id
