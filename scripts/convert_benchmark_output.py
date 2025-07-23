#!/usr/bin/env python3
"""
Convert Criterion benchmark output to bencher format for GitHub Actions benchmark tracking.

This script parses Criterion's output format and converts it to the bencher format
expected by benchmark-action/github-action-benchmark.

Usage:
    python3 convert_benchmark_output.py <input_file> <output_file>
    python3 convert_benchmark_output.py benchmark-output.txt output.txt
"""

import argparse
import re
import sys


def convert_unit_to_nanoseconds(value: float, unit: str) -> int:
    """Convert time value from various units to nanoseconds."""
    unit_multipliers = {
        "ms": 1_000_000,
        "us": 1_000,
        "µs": 1_000,  # Alternative microsecond symbol
        "ns": 1,
        "s": 1_000_000_000,
    }

    multiplier = unit_multipliers.get(unit.lower())
    if multiplier is None:
        raise ValueError(f"Unknown time unit: {unit}")

    return int(value * multiplier)


def parse_criterion_output(content: str) -> list:
    """Parse Criterion benchmark output and extract timing information."""
    # Pattern to match Criterion output
    # Example: "benchmark_name    time:   [1.2345 ms 1.2567 ms 1.2789 ms]"
    pattern = r"(\S+)\s+time:\s+\[(\d+\.?\d*)\s+(\w+)\s+(\d+\.?\d*)\s+(\w+)\s+(\d+\.?\d*)\s+(\w+)\]"

    results = []

    for match in re.finditer(pattern, content):
        test_name = match.group(1)

        # Extract the three timing values (low, median, high estimates)
        low_value = float(match.group(2))
        low_unit = match.group(3)
        median_value = float(match.group(4))
        median_unit = match.group(5)
        high_value = float(match.group(6))
        high_unit = match.group(7)

        # Use the median value as the primary benchmark result
        try:
            value_ns = convert_unit_to_nanoseconds(median_value, median_unit)

            # Calculate variance estimate from low and high values
            low_ns = convert_unit_to_nanoseconds(low_value, low_unit)
            high_ns = convert_unit_to_nanoseconds(high_value, high_unit)
            variance = max(abs(value_ns - low_ns), abs(high_ns - value_ns))

            results.append({"name": test_name, "value": value_ns, "variance": variance})

        except ValueError as e:
            print(f"Warning: Skipping benchmark '{test_name}': {e}", file=sys.stderr)
            continue

    return results


def write_bencher_format(results: list, output_file: str):
    """Write benchmark results in bencher format."""
    with open(output_file, "w") as f:
        for result in results:
            # Format: test <name> ... bench: <value> ns/iter (+/- <variance>)
            f.write(
                f"test {result['name']} ... bench: {result['value']:,} ns/iter "
                f"(+/- {result['variance']:,})\n"
            )


def main():
    parser = argparse.ArgumentParser(
        description="Convert Criterion benchmark output to bencher format"
    )
    parser.add_argument(
        "input_file", help="Input file containing Criterion benchmark output"
    )
    parser.add_argument("output_file", help="Output file for bencher format results")
    parser.add_argument(
        "--verbose", "-v", action="store_true", help="Enable verbose output"
    )

    args = parser.parse_args()

    try:
        # Read input file
        with open(args.input_file, "r", encoding="utf-8") as f:
            content = f.read()

        if args.verbose:
            print(f"Reading benchmark output from: {args.input_file}")

        # Parse Criterion output
        results = parse_criterion_output(content)

        if not results:
            print("Warning: No benchmark results found in input file", file=sys.stderr)
            return 1

        if args.verbose:
            print(f"Found {len(results)} benchmark results")
            for result in results:
                print(f"  {result['name']}: {result['value']:,} ns/iter")

        # Write bencher format output
        write_bencher_format(results, args.output_file)

        if args.verbose:
            print(f"Results written to: {args.output_file}")

        return 0

    except FileNotFoundError:
        print(f"Error: Input file '{args.input_file}' not found", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
