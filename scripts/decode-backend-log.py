#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Dict
from urllib.parse import unquote


def load_catalog(repo_root: Path) -> dict:
    catalog_path = repo_root / "backend" / "log_catalog.json"
    with catalog_path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def invert_map(values: Dict[str, str]) -> Dict[str, str]:
    return {value: key for key, value in values.items()}


def decode_token(value: str) -> str:
    if value.startswith("~"):
        return unquote(value[1:])
    return unquote(value)


def expand_timestamp(value: str) -> str:
    if len(value) == 23 and value[8] == "T" and value.endswith("Z"):
        return (
            f"{value[0:4]}-{value[4:6]}-{value[6:8]}T"
            f"{value[9:11]}:{value[11:13]}:{value[13:15]}{value[15:-1]}Z"
        )
    return value


def decode_line(
    line: str,
    targets_rev: Dict[str, str],
    events_rev: Dict[str, str],
    fields_rev: Dict[str, str],
    value_maps_rev: Dict[str, Dict[str, str]],
) -> str:
    parts = line.lstrip("\ufeff").rstrip("\n").split("|")
    if len(parts) < 4:
        return line.rstrip("\n")

    timestamp_raw, level_code, target_code, event_code = parts[:4]
    timestamp = expand_timestamp(timestamp_raw)
    level = {
        "E": "ERROR",
        "W": "WARN",
        "I": "INFO",
        "D": "DEBUG",
        "T": "TRACE",
    }.get(level_code, level_code)
    target = targets_rev.get(target_code, decode_token(target_code))
    message_key = events_rev.get(event_code)
    if message_key:
        _, message = message_key.split("|", 1)
    else:
        message = decode_token(event_code)

    decoded_fields = []
    for field_part in parts[4:]:
        if "=" not in field_part:
            decoded_fields.append(decode_token(field_part))
            continue

        field_code, encoded_value = field_part.split("=", 1)
        field_name = fields_rev.get(field_code, decode_token(field_code))
        field_value = value_maps_rev.get(field_name, {}).get(encoded_value, decode_token(encoded_value))
        decoded_fields.append(f"{field_name}={field_value}")

    suffix = f" {' '.join(decoded_fields)}" if decoded_fields else ""
    return f"{timestamp}  {level:<5} {target}: {message}{suffix}"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Decode compact Anicargo backend log files into human-readable text.",
    )
    parser.add_argument("input", help="Path to the compact log file.")
    parser.add_argument(
        "-o",
        "--output",
        help="Optional output file path. Defaults to <input>.decoded.log",
    )
    args = parser.parse_args()

    input_path = Path(args.input).resolve()
    if not input_path.exists():
        raise SystemExit(f"Input log file does not exist: {input_path}")

    repo_root = Path(__file__).resolve().parent.parent
    catalog = load_catalog(repo_root)
    targets_rev = invert_map(catalog["targets"])
    events_rev = invert_map(catalog["events"])
    fields_rev = invert_map(catalog["fields"])
    value_maps_rev = {
        field_name: invert_map(values) for field_name, values in catalog.get("value_maps", {}).items()
    }

    output_path = (
        Path(args.output).resolve()
        if args.output
        else input_path.with_suffix(input_path.suffix + ".decoded.log")
    )

    with input_path.open("r", encoding="utf-8", errors="replace") as source:
        with output_path.open("w", encoding="utf-8") as destination:
            for line in source:
                destination.write(
                    decode_line(
                        line,
                        targets_rev,
                        events_rev,
                        fields_rev,
                        value_maps_rev,
                    )
                )
                destination.write("\n")

    print(f"Decoded log written to {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
