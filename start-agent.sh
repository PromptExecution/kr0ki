#!/bin/bash
# Start the storyb00k agent server locally
# This script starts the agent server that connects the playbook UI to the LLM

set -e

cd "$(dirname "$0")/containers/kr0ki-storyb00k-agent"

# Create virtual environment if it doesn't exist
if [ ! -d ".venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv .venv
    .venv/bin/pip install -q -r requirements.txt
fi

# Detect hostname and IP for CORS configuration
HOSTNAME=$(hostname)
IP_ADDR=$(hostname -I | awk '{print $1}')

# Set environment variables
export KR0KI_URL="${KR0KI_URL:-http://${HOSTNAME}:8787}"
export KR0KI_STORYB00K_PORT="${KR0KI_STORYB00K_PORT:-8789}"

# LLM configuration - use the b00t-heretic container on port 8002
export OPENAI_API_URL="${OPENAI_API_URL:-http://${HOSTNAME}:8002/v1}"
export OPENAI_API_KEY="${OPENAI_API_KEY:-dummy-key}"  # llama.cpp doesn't require a real key

# CORS configuration - allow access from multiple origins
# Include localhost, 127.0.0.1, hostname, and IP address
export KR0KI_STORYB00K_ALLOWED_ORIGINS="${KR0KI_STORYB00K_ALLOWED_ORIGINS:-http://127.0.0.1:8787,http://localhost:8787,http://${HOSTNAME}:8787,http://${IP_ADDR}:8787}"

echo "Starting storyb00k agent server..."
echo "  KR0KI_URL: $KR0KI_URL"
echo "  KR0KI_STORYB00K_PORT: $KR0KI_STORYB00K_PORT"
echo "  OPENAI_API_URL: $OPENAI_API_URL"
echo "  ALLOWED_ORIGINS: $KR0KI_STORYB00K_ALLOWED_ORIGINS"
echo ""
echo "Press Ctrl+C to stop"
echo ""

.venv/bin/python3 server.py
