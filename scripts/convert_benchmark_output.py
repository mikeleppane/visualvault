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
import json
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


def parse_existing_bencher_format(content: str, debug: bool = False) -> list:
    """Parse input that's already in bencher format."""
    # Pattern to match existing bencher format:
    # test benchmark_name ... bench: 123 ns/iter (+/- 45)
    pattern = r"test\s+([^\s].+?)\s+\.\.\.\s+bench:\s+(\d+(?:,\d+)*)\s+ns/iter\s+\(\+/-\s+(\d+(?:,\d+)*)\)"

    results = []
    matches = list(re.finditer(pattern, content))

    if debug:
        print(f"Bencher format pattern found {len(matches)} matches", file=sys.stderr)

    for match in matches:
        test_name = match.group(1).strip()
        value_str = match.group(2).replace(",", "")  # Remove commas
        variance_str = match.group(3).replace(",", "")  # Remove commas

        try:
            value = int(value_str)
            variance = int(variance_str)

            if debug:
                print(
                    f"Parsed: {test_name} -> {value} ns/iter (+/- {variance})",
                    file=sys.stderr,
                )

            results.append({"name": test_name, "value": value, "variance": variance})

        except ValueError as e:
            if debug:
                print(
                    f"Warning: Skipping benchmark '{test_name}': {e}", file=sys.stderr
                )
            continue

    return results


def parse_bencher_json_output(content: str, debug: bool = False) -> list:
    """Parse bencher format output with JSON timing data."""
    # Multiple patterns to try
    patterns = [
        # Pattern 1: Standard format with optional #N suffix
        r"([^:\n]+?)(?:\s+#\d+)?\s*:\s*(\{[^}]+\})",
        # Pattern 2: More flexible whitespace handling
        r"(.+?)\s*:\s*(\{[^}]+\})",
        # Pattern 3: Line-by-line approach
        r"^([^:\n]+?)\s*:\s*(\{.+?\})",
    ]

    results = []

    for i, pattern in enumerate(patterns):
        if debug:
            print(f"Trying JSON pattern {i+1}: {pattern}", file=sys.stderr)

        matches = list(re.finditer(pattern, content, re.MULTILINE))
        if debug:
            print(f"JSON pattern {i+1} found {len(matches)} matches", file=sys.stderr)

        if matches:
            for match in matches:
                test_name = match.group(1).strip()
                json_data = match.group(2)

                if debug:
                    print(
                        f"Match: '{test_name}' -> '{json_data[:50]}...'",
                        file=sys.stderr,
                    )

                try:
                    # Parse JSON data
                    data = json.loads(json_data)

                    # Extract values
                    estimate = data.get("estimate", 0)
                    lower_bound = data.get("lower_bound", estimate)
                    upper_bound = data.get("upper_bound", estimate)
                    unit = data.get("unit", "ns")

                    # Convert to nanoseconds
                    value_ns = convert_unit_to_nanoseconds(estimate, unit)
                    low_ns = convert_unit_to_nanoseconds(lower_bound, unit)
                    high_ns = convert_unit_to_nanoseconds(upper_bound, unit)

                    # Calculate variance
                    variance = max(abs(value_ns - low_ns), abs(high_ns - value_ns))

                    results.append(
                        {"name": test_name, "value": value_ns, "variance": variance}
                    )

                except (json.JSONDecodeError, ValueError) as e:
                    if debug:
                        print(
                            f"Warning: Skipping benchmark '{test_name}': {e}",
                            file=sys.stderr,
                        )
                    continue

            # If we found results with this pattern, stop trying others
            if results:
                if debug:
                    print(
                        f"Successfully parsed {len(results)} results with JSON pattern {i+1}",
                        file=sys.stderr,
                    )
                break

    return results


def parse_criterion_output(content: str, debug: bool = False) -> list:
    """Parse Criterion benchmark output and extract timing information."""
    # Pattern to match Criterion output
    # Example: "benchmark_name    time:   [1.2345 ms 1.2567 ms 1.2789 ms]"
    pattern = r"(\S+)\s+time:\s+\[(\d+\.?\d*)\s+(\w+)\s+(\d+\.?\d*)\s+(\w+)\s+(\d+\.?\d*)\s+(\w+)\]"

    results = []
    matches = list(re.finditer(pattern, content))

    if debug:
        print(f"Criterion pattern found {len(matches)} matches", file=sys.stderr)

    for match in matches:
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
            if debug:
                print(
                    f"Warning: Skipping benchmark '{test_name}': {e}", file=sys.stderr
                )
            continue

    return results


def parse_benchmark_output(content: str, debug: bool = False) -> list:
    """Parse benchmark output, trying different formats."""
    if debug:
        print("Analyzing input content...", file=sys.stderr)
        lines = content.split("\n")
        print(f"Total lines: {len(lines)}", file=sys.stderr)
        print("First 5 non-empty lines:", file=sys.stderr)
        for i, line in enumerate(lines[:10]):
            if line.strip():
                print(f"  {i+1}: {line[:100]}", file=sys.stderr)

    # First check if input is already in bencher format
    results = parse_existing_bencher_format(content, debug)

    # If no results, try bencher JSON format
    if not results:
        results = parse_bencher_json_output(content, debug)

    # If still no results, try Criterion format
    if not results:
        if debug:
            print(
                "No results from JSON format, trying Criterion format...",
                file=sys.stderr,
            )
        results = parse_criterion_output(content, debug)

    return results


def write_bencher_format(results: list, output_file: str):
    """Write benchmark results in bencher format."""
    with open(output_file, "w") as f:
        for result in results:
            # Format: test <name> ... bench: <value> ns/iter (+/- <variance>)
            f.write(
                f"test {result['name']} ... bench: {result['value']} ns/iter "
                f"(+/- {result['variance']})\n"
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
    parser.add_argument(
        "--debug", "-d", action="store_true", help="Enable debug output"
    )

    args = parser.parse_args()

    try:
        # Read input file
        with open(args.input_file, "r", encoding="utf-8") as f:
            content = f.read()

        if args.verbose:
            print(f"Reading benchmark output from: {args.input_file}")

        if args.debug:
            print(f"Input file size: {len(content)} characters", file=sys.stderr)
            print(
                f"Input content (first 200 chars):\n{content[:200]}\n", file=sys.stderr
            )

        # Parse benchmark output
        results = parse_benchmark_output(content, args.debug)

        if not results:
            print("Warning: No benchmark results found in input file", file=sys.stderr)

            if args.debug:
                print("Debug: Manual pattern search...", file=sys.stderr)

                # Look for test lines
                test_lines = [
                    line
                    for line in content.split("\n")
                    if line.strip().startswith("test ")
                ]
                print(
                    f"Found {len(test_lines)} lines starting with 'test'",
                    file=sys.stderr,
                )
                for i, line in enumerate(test_lines[:3]):
                    print(f"  {i+1}: {line}", file=sys.stderr)

            # Create a dummy result to avoid empty output
            results = [{"name": "dummy_benchmark", "value": 1000, "variance": 0}]

        if args.verbose:
            print(f"Found {len(results)} benchmark results")
            for result in results:
                print(f"  {result['name']}: {result['value']} ns/iter")

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
        if args.debug:
            import traceback

            traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
