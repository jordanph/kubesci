#!/bin/sh

set -euo pipefail

echo "Cloning down the repo..."

mkdir repo

git clone $REPO_URL repo

cd repo

git checkout $COMMIT_SHA

echo "Successfully cloned the repo! Copying files to container volumes..."

export IFS=";"

for i in $CONTAINER_VOLUMES; do
    cp -a . $i/
done

echo "Fnished copying repo into the relevant volumes. Shutting down..."
