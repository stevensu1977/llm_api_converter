//! Python runner script for PTC sandbox
//!
//! This module contains the Python runner script that gets injected into
//! the sandbox container for executing Claude-generated code.

/// The Python runner script that handles code execution and tool calls.
///
/// This script:
/// 1. Reads code from stdin or a file
/// 2. Executes the code in a controlled environment
/// 3. Intercepts tool calls and writes them to a special file
/// 4. Waits for tool results from a results file
/// 5. Continues execution with the tool results
pub const RUNNER_SCRIPT: &str = r#"#!/usr/bin/env python3
"""
PTC Runner Script - Executes code and handles tool calls

This script is injected into the sandbox container and handles:
- Code execution
- Tool call interception
- Result injection
- Communication with the proxy via files
"""

import sys
import json
import os
import time
import traceback
from typing import Any, Dict, List, Optional
import asyncio

# File paths for IPC
TOOL_CALLS_FILE = "/tmp/tool_calls.json"
TOOL_RESULTS_FILE = "/tmp/tool_results.json"
STATUS_FILE = "/tmp/status.json"
READY_FILE = "/tmp/ready"

# Batch window for collecting parallel tool calls (ms)
BATCH_WINDOW_MS = 100


class ToolCallBatcher:
    """Collects tool calls within a batch window for parallel execution."""

    def __init__(self, batch_window_ms: int = BATCH_WINDOW_MS):
        self.batch_window_ms = batch_window_ms
        self.pending_calls: List[Dict] = []
        self.last_call_time: Optional[float] = None

    def add_call(self, tool_use_id: str, name: str, input_data: Dict) -> None:
        """Add a tool call to the batch."""
        self.pending_calls.append({
            "tool_use_id": tool_use_id,
            "name": name,
            "input": input_data
        })
        self.last_call_time = time.time()

    def should_flush(self) -> bool:
        """Check if the batch window has elapsed."""
        if not self.last_call_time:
            return False
        elapsed_ms = (time.time() - self.last_call_time) * 1000
        return elapsed_ms >= self.batch_window_ms

    def flush(self) -> List[Dict]:
        """Get and clear pending calls."""
        calls = self.pending_calls
        self.pending_calls = []
        self.last_call_time = None
        return calls


class ToolCallHandler:
    """Handles tool calls by writing to file and waiting for results."""

    def __init__(self):
        self.call_counter = 0
        self.batcher = ToolCallBatcher()
        self.results_cache: Dict[str, Any] = {}

    def generate_tool_use_id(self) -> str:
        """Generate a unique tool use ID."""
        self.call_counter += 1
        return f"toolu_{self.call_counter:012d}"

    def request_tool_call(self, name: str, **kwargs) -> Any:
        """Request a tool call and wait for the result."""
        tool_use_id = self.generate_tool_use_id()

        # Add to batch
        self.batcher.add_call(tool_use_id, name, kwargs)

        # Wait a bit for potential parallel calls
        time.sleep(self.batcher.batch_window_ms / 1000.0)

        # Flush and write all pending calls
        if self.batcher.should_flush():
            calls = self.batcher.flush()
            self._write_tool_calls(calls)

            # Wait for results
            results = self._wait_for_results([c["tool_use_id"] for c in calls])
            self.results_cache.update(results)

        return self.results_cache.get(tool_use_id)

    def _write_tool_calls(self, calls: List[Dict]) -> None:
        """Write tool calls to the IPC file."""
        with open(TOOL_CALLS_FILE, 'w') as f:
            json.dump({
                "type": "tool_calls",
                "calls": calls
            }, f)

        # Write status to signal we're waiting
        with open(STATUS_FILE, 'w') as f:
            json.dump({
                "status": "waiting_for_tools",
                "pending_ids": [c["tool_use_id"] for c in calls]
            }, f)

    def _wait_for_results(self, tool_use_ids: List[str], timeout: int = 300) -> Dict[str, Any]:
        """Wait for tool results from the proxy."""
        start_time = time.time()
        results = {}

        while len(results) < len(tool_use_ids):
            if time.time() - start_time > timeout:
                raise TimeoutError(f"Timeout waiting for tool results")

            if os.path.exists(TOOL_RESULTS_FILE):
                try:
                    with open(TOOL_RESULTS_FILE, 'r') as f:
                        data = json.load(f)

                    if data.get("type") == "tool_results":
                        for result in data.get("results", []):
                            tool_id = result.get("tool_use_id")
                            if tool_id in tool_use_ids:
                                results[tool_id] = result.get("content")

                    # Clear the file after reading
                    os.remove(TOOL_RESULTS_FILE)

                except json.JSONDecodeError:
                    pass
                except Exception as e:
                    print(f"Error reading results: {e}", file=sys.stderr)

            time.sleep(0.1)  # Poll every 100ms

        # Update status
        with open(STATUS_FILE, 'w') as f:
            json.dump({"status": "running"}, f)

        return results


# Global tool call handler
_handler = ToolCallHandler()


def call_tool(name: str, **kwargs) -> Any:
    """Call a tool and wait for the result.

    This function is called by Claude-generated code to invoke tools.
    It writes the tool call to a file and waits for the result.
    """
    return _handler.request_tool_call(name, **kwargs)


# Async version for asyncio.gather support
async def async_call_tool(name: str, **kwargs) -> Any:
    """Async version of call_tool for parallel execution."""
    # Run synchronously since we're doing file-based IPC
    return call_tool(name, **kwargs)


def create_tool_function(name: str):
    """Create a tool function that can be called by generated code."""
    def tool_func(**kwargs):
        return call_tool(name, **kwargs)
    return tool_func


def execute_code(code: str, tools: List[str] = None) -> Dict:
    """Execute the provided code with tool support.

    Args:
        code: Python code to execute
        tools: List of tool names to make available

    Returns:
        Dict with stdout, stderr, and exit_code
    """
    import io
    from contextlib import redirect_stdout, redirect_stderr

    # Capture stdout and stderr
    stdout_capture = io.StringIO()
    stderr_capture = io.StringIO()

    # Create execution namespace with tool functions
    namespace = {
        '__builtins__': __builtins__,
        'call_tool': call_tool,
        'async_call_tool': async_call_tool,
        'asyncio': asyncio,
    }

    # Add named tool functions
    if tools:
        for tool_name in tools:
            namespace[tool_name] = create_tool_function(tool_name)

    exit_code = 0

    try:
        with redirect_stdout(stdout_capture), redirect_stderr(stderr_capture):
            exec(code, namespace)
    except Exception as e:
        exit_code = 1
        stderr_capture.write(f"\n{traceback.format_exc()}")

    return {
        "stdout": stdout_capture.getvalue(),
        "stderr": stderr_capture.getvalue(),
        "exit_code": exit_code
    }


def main():
    """Main entry point for the runner script."""
    # Signal that we're ready
    with open(READY_FILE, 'w') as f:
        f.write("ready")

    with open(STATUS_FILE, 'w') as f:
        json.dump({"status": "ready"}, f)

    # Read code from stdin or file argument
    if len(sys.argv) > 1:
        code_file = sys.argv[1]
        with open(code_file, 'r') as f:
            code = f.read()
    else:
        code = sys.stdin.read()

    # Read tools from environment or second argument
    tools = []
    if len(sys.argv) > 2:
        tools = sys.argv[2].split(',')
    elif os.environ.get('PTC_TOOLS'):
        tools = os.environ['PTC_TOOLS'].split(',')

    # Execute the code
    result = execute_code(code, tools)

    # Write final status
    with open(STATUS_FILE, 'w') as f:
        json.dump({
            "status": "completed",
            "exit_code": result["exit_code"]
        }, f)

    # Output results
    sys.stdout.write(result["stdout"])
    sys.stderr.write(result["stderr"])
    sys.exit(result["exit_code"])


if __name__ == "__main__":
    main()
"#;

/// Get the runner script as bytes for copying to container.
pub fn get_runner_script_bytes() -> Vec<u8> {
    RUNNER_SCRIPT.as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_script_not_empty() {
        assert!(!RUNNER_SCRIPT.is_empty());
        assert!(RUNNER_SCRIPT.contains("def execute_code"));
        assert!(RUNNER_SCRIPT.contains("def call_tool"));
    }

    #[test]
    fn test_runner_script_bytes() {
        let bytes = get_runner_script_bytes();
        assert!(!bytes.is_empty());
        // Should be valid UTF-8
        assert!(std::str::from_utf8(&bytes).is_ok());
    }

    #[test]
    fn test_runner_has_main() {
        assert!(RUNNER_SCRIPT.contains("def main():"));
        assert!(RUNNER_SCRIPT.contains("if __name__ == \"__main__\":"));
    }

    #[test]
    fn test_runner_has_ipc_files() {
        assert!(RUNNER_SCRIPT.contains("TOOL_CALLS_FILE"));
        assert!(RUNNER_SCRIPT.contains("TOOL_RESULTS_FILE"));
        assert!(RUNNER_SCRIPT.contains("STATUS_FILE"));
    }
}
