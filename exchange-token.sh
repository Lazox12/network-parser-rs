#!/bin/bash
set -e

# Exchange JWT token
echo "Exchanging OIDC token..." >&2
RESPONSE=$(curl -s -X POST https://crates.io/api/v1/trusted_publishing/tokens \
  -H "Content-Type: application/json" \
  -H "User-Agent: gitlab-trusted-publishing (your@email.com)" \
  -d "{\"jwt\": \"$CRATES_IO_ID_TOKEN\"}")

# Extract publish token
CRATES_IO_PUBLISH_TOKEN=$(echo "$RESPONSE" | jq -r '.token')

if [ "$CRATES_IO_PUBLISH_TOKEN" = "null" ] || [ -z "$CRATES_IO_PUBLISH_TOKEN" ]; then
  echo "Failed to get upload token" >&2
  echo "$RESPONSE" >&2
  exit 1
fi

echo "$CRATES_IO_PUBLISH_TOKEN"