"""
Formatter mixin and utilities for Zymatica CLI.
"""

from typing import Any

def format_duration_compact(seconds: float) -> str:
    """Format duration in seconds to a human-readable compact string."""
    if seconds < 0.001:
        return f"{seconds * 1_000_000:.0f}µs"
    elif seconds < 1.0:
        return f"{seconds * 1000:.1f}ms"
    elif seconds < 60:
        return f"{seconds:.1f}s"
    minutes = int(seconds // 60)
    rem_seconds = seconds % 60
    return f"{minutes}m{rem_seconds:.0f}s"

def format_token_count_compact(tokens: int) -> str:
    """Format token count to compact string."""
    if tokens < 1000:
        return str(tokens)
    elif tokens < 1_000_000:
        return f"{tokens / 1000:.1f}k"
    else:
        return f"{tokens / 1_000_000:.2f}M"

class CLIFormatterMixin:
    """Provides formatting utilities for CLI output."""
    def format_duration(self, seconds: float) -> str:
        return format_duration_compact(seconds)

    def format_tokens(self, tokens: int) -> str:
        return format_token_count_compact(tokens)
