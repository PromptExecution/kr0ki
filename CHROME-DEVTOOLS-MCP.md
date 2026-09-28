# Chrome DevTools MCP Integration - 2026-09-27

## Summary

Successfully integrated the official Chrome DevTools MCP server using the proper b00t datum workflow.

## Proper b00t Workflow

### 1. Created b00t Datum

Created `/home/brianh/.dotfiles/_b00t_/chrome-devtools-mcp.mcp.toml`:

```toml
[b00t]
name = "chrome-devtools-mcp"
type = "mcp"
hint = "Official Chrome DevTools MCP server — automation, debugging, performance analysis, and screenshots via Chrome DevTools Protocol. Connects to Chrome at 192.168.1.150:9222"

[[b00t.gate]]
command = "npx"
args = ["-y", "chrome-devtools-mcp@latest", "--help"]

[[b00t.mcp.stdio]]
transport = "stdio"
command = "npx"
args = ["-y", "chrome-devtools-mcp@latest", "--browser-url=http://192.168.1.150:9222"]
priority = 0
requires = ["node", "internet"]
```

### 2. Installed Using b00t

```bash
b00t mcp install chrome-devtools-mcp dotmcpjson
```

This properly installed the MCP server to `.mcp.json` using the datum.

### 3. Result

The `.mcp.json` now contains:

```json
{
  "mcpServers": {
    "chrome-devtools-mcp": {
      "args": [
        "-y",
        "chrome-devtools-mcp@latest",
        "--browser-url=http://192.168.1.150:9222"
      ],
      "command": "npx"
    },
    "kr0ki-mcp": {
      "args": [
        "--context",
        "Default",
        "-n",
        "kr0ki",
        "exec",
        "-i",
        "pod/kr0ki-local",
        "-c",
        "kr0ki-mcp",
        "--",
        "python3",
        "/opt/kr0ki-mcp/bridge.py"
      ],
      "command": "kubectl"
    }
  }
}
```

## Why This Approach is Correct

### ✅ Proper Pattern
1. **Datum-driven**: Configuration is defined in a reusable b00t datum
2. **Version-controlled**: The datum is in `~/.dotfiles/_b00t_/` (tracked in dotfiles repo)
3. **Reproducible**: Any project can install with `b00t mcp install chrome-devtools-mcp dotmcpjson`
4. **Consistent**: Uses the same workflow as all other MCP servers in the b00t ecosystem

### ❌ Antipattern (What I Did Initially)
1. **Manual editing**: Directly editing `.mcp.json` bypasses the datum system
2. **Not reusable**: Configuration is project-specific, not in the datum
3. **Inconsistent**: Different from how other MCP servers are managed
4. **Not traceable**: No datum file to reference or modify

## Browser Host Configuration

- **Browser Host**: `192.168.1.150`
- **Debug Port**: `9222` (default Chrome DevTools remote debugging port)
- **Connection Method**: `--browser-url` (connects to a running Chrome instance)

## Prerequisites

### 1. Chrome Browser on 192.168.1.150

The Chrome browser must be started with remote debugging enabled:

```bash
# On the machine at 192.168.1.150
google-chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-profile
```

Or on macOS:
```bash
/Applications/Google\ Chrome.app/Contents/MacOS/Google\ Chrome --remote-debugging-port=9222 --user-data-dir=/tmp/chrome-profile
```

Or on Windows:
```cmd
"C:\Program Files\Google\Chrome\Application\chrome.exe" --remote-debugging-port=9222 --user-data-dir="%TEMP%\chrome-profile"
```

### 2. Network Access

The machine running the MCP client must have network access to `192.168.1.150:9222`.

### 3. Node.js and npx

The MCP server requires Node.js and npx to be installed:
- Node.js: ✅ Installed (v22.15.1)
- npx: ✅ Installed (v10.9.2)

## Usage

Once configured, MCP clients (like Claude Code, Cursor, etc.) can use the chrome-devtools MCP server to:

- Navigate pages
- Take screenshots
- Execute JavaScript
- Inspect network requests
- Record performance traces
- Debug applications
- And much more

Example prompt to test:
```
Take a screenshot of http://192.168.1.137:8787/playbook/
```

## Security Considerations

⚠️ **Warning**: Enabling the remote debugging port opens up a debugging port on the running browser instance. Any application on the network can connect to this port and control the browser.

**Recommendations**:
1. Only enable remote debugging when needed
2. Use a separate user data directory (as shown above)
3. Don't browse sensitive websites while the debugging port is open
4. Consider firewall rules to restrict access to port 9222

## Reinstalling in Other Projects

To use this MCP server in other projects:

```bash
cd /path/to/project
b00t mcp install chrome-devtools-mcp dotmcpjson
```

This will add the chrome-devtools-mcp server to that project's `.mcp.json` with the same configuration.

## Modifying the Configuration

To change the browser URL or other parameters:

1. Edit the datum: `~/.dotfiles/_b00t_/chrome-devtools-mcp.mcp.toml`
2. Reinstall: `b00t mcp install chrome-devtools-mcp dotmcpjson`

Or for project-specific overrides, edit the project's `.mcp.json` directly (but note this won't be tracked in the datum).

## Related Documentation

- [Chrome DevTools MCP GitHub](https://github.com/ChromeDevTools/chrome-devtools-mcp)
- [Configuration Guide](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/configuration.md)
- [Advanced Usage](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/advanced-usage.md)
- [Tool Reference](https://github.com/ChromeDevTools/chrome-devtools-mcp/blob/main/docs/tool-reference.md)

## Files Modified

- `~/.dotfiles/_b00t_/chrome-devtools-mcp.mcp.toml` - Created b00t datum
- `.mcp.json` - Installed via `b00t mcp install chrome-devtools-mcp dotmcpjson`

## Lessons Learned

**Always use b00t datums for MCP server configuration:**
1. Create the datum in `~/.dotfiles/_b00t_/`
2. Install with `b00t mcp install <name> dotmcpjson`
3. Never manually edit `.mcp.json` unless absolutely necessary

This ensures consistency, reusability, and proper configuration management across all projects.
