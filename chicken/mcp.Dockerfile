# Apollo GraphOS Credentials for MCP Server
# These credentials connect your MCP server to Apollo GraphOS
#
# Usage:
# - For VSCode: These variables are automatically loaded
# - For manual runs: Source this file before running rover dev
#   Linux/macOS: set -a && source .env && set +a && rover dev --mcp .apollo/mcp.local.yaml
#   Windows: Get-Content .env | ForEach-Object { $name, $value = $_.split('=',2); [System.Environment]::SetEnvironmentVariable($name, $value) }; rover dev --mcp .apollo/mcp.local.yaml

# =============================================================================
# Apollo MCP Server Dockerfile
# =============================================================================
#
# This Dockerfile sets up the Apollo MCP (Model Context Protocol) Server
# to expose your GraphQL API as tools that AI assistants like Claude can use.
#
# The Apollo MCP Server acts as a bridge between your GraphQL API and MCP
# clients, automatically converting GraphQL operations into callable tools.
#
# RECOMMENDED USE:
# - Development and testing with MCP Inspector or Claude Desktop
# - Local development and prototyping
#
# USAGE:
# 1. Build: docker build -f mcp.Dockerfile -t My API .
# 2. Run:   docker run -p 5050:5050 --env-file .env My API
# 3. Test:  npx @mcp/inspector (Transport: HTTP, URL: http://localhost:5050/mcp)
#
# =============================================================================

FROM ghcr.io/apollographql/apollo-mcp-server:latest

# =============================================================================
# METADATA AND LABELS
# =============================================================================

# Add metadata about this image
LABEL org.opencontainers.image.title="My API MCP Server"
LABEL org.opencontainers.image.description="Apollo MCP Server for My API GraphQL API"
LABEL org.opencontainers.image.vendor="Apollo GraphQL"

# =============================================================================
# COPY APPLICATION FILES
# =============================================================================

# Note: base image is pre-configured with appropriate user restrictions for security

# Copy MCP configuration file to container root (where server expects it)
COPY .apollo/mcp.local.yaml /mcp.yaml

# Copy any custom schema files or additional configuration
# COPY schema.graphql /app/schema.graphql
# COPY custom-resolvers.js /app/custom-resolvers.js

# =============================================================================
# RUNTIME CONFIGURATION
# =============================================================================

# Note: Base image is pre-configured with proper non-root user security

# Expose the MCP server port
# The MCP server listens on this port for connections from AI assistants
EXPOSE 5050

# =============================================================================
# STARTUP COMMAND
# =============================================================================

# Start the Apollo MCP Server
# The server will:
# 1. Auto-discover configuration files in current directory
# 2. Load tools from MCP_TOOLS_DIR (/app/tools)
# 3. Connect to GRAPHQL_ENDPOINT
# 4. Start MCP server on MCP_PORT (5050)
CMD ["mcp.yaml"]

# =============================================================================
# TROUBLESHOOTING
# =============================================================================
#
# Common issues and solutions:
#
# 1. Connection refused to GraphQL endpoint:
#    - Ensure GRAPHQL_ENDPOINT is accessible from the container
#    - Check if GraphQL server is running
#    - Verify network connectivity between containers
#
# 2. Tools not loading:
#    - Ensure .graphql files are in the tools/ directory
#    - Check file permissions and ownership
#    - Verify GraphQL syntax in tool files
#
# 3. Authentication failures:
#    - Set APOLLO_KEY environment variable
#    - Verify API key has correct permissions
#    - Check if GraphQL endpoint requires authentication
#
# 4. MCP server not starting:
#    - Check configuration file syntax
#    - Verify all required environment variables are set
#    - Look at container logs: docker logs <container_name>
#
# =============================================================================