#!/usr/bin/env python3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from comment_checker.python_rules import RULES, main

__all__ = ["RULES", "main"]

if __name__ == "__main__":
    sys.exit(main())
