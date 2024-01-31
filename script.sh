#!/bin/bash

for arg in "$@"; do
    echo "$arg" >> .env
done

cargo test

cinema-score
