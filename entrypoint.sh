#!/bin/bash
if [ "$SERVICE_TYPE" = "api" ]; then
    echo "Starting api..."
    exec api
else
    echo "Starting agent..."
    exec agent
fi
