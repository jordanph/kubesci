# Init Containers

This directory houses the Docker images that are used as `init-containers` when spinning up the Pod in Kubernetes.

## Git-checkout

This Docker image is used to checkout the repo and commit corresponding to the `git push` used to set off the build. It also is responsible for copying the files accross to the volumes of the containers (steps) in the Pod.
