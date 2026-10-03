#!/usr/bin/env python3
"""CLI entry point for generating deterministic synthetic Takeout media test fixtures."""

import sys
from pathlib import Path

# Add backend directory to sys.path so core modules can be resolved
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "backend"))

from core.fixture_generator import main

if __name__ == "__main__":
    main()
