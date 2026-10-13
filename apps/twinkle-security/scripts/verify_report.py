#!/usr/bin/env python3
"""Verify a completed benchmark report against the 10 Hz workload contract."""
import argparse
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("report", type=Path)
args = parser.parse_args()
with args.report.open() as stream:
    events = [json.loads(line) for line in stream if line.strip()]
metadata = next(event for event in events if event["event"] == "metadata")
summary = events[-1]
assert summary["event"] == "summary", "missing final summary"
records = [event for event in events if event["event"] == "tick"]
assert len(records) == summary["accepted"] == metadata["ticks"], "incomplete workload"
assert summary["processed"] == len(records)
services, groups = {"standard": (60, 60), "dense": (240, 120), "stress": (1000, 200)}[metadata["preset"]]
expected_rows = [1800, groups * 100, services, 20]
headless = metadata["mode"].startswith("headless")
for index, record in enumerate(records):
    assert record["epoch"] == 1
    assert record["tick"] == metadata["start_tick"] + index + 1
    assert record["scheduled_us"] == index * 100_000
    assert record["admitted_rows"] == expected_rows
    assert record["changed_heat_cells"] is not None and 0 <= record["changed_heat_cells"] <= groups * 100
    assert record["accepted_us"] >= record["dispatch_us"]
    if index:
        assert record["revision"] > records[index - 1]["revision"]
    for panel_index, panel in enumerate(record["panels"]):
        assert panel["input_rows"] == expected_rows[panel_index]
        assert panel["paint_commands"] > 0
    heat = record["panels"][1]
    assert heat["rectangles_including_background"] >= groups * 100 + 1
    assert heat["displayed_rows"] == groups * 100
    if not headless:
        width, height = heat["plot_logical_size"]
        assert width >= 599.99 and height >= 299.99
        assert all(record[key] is not None for key in ["host_update_us", "declaration_us", "layout_us"])
    if headless:
        assert record["submitted_us"] is None and record["presented_us"] is None
assert summary["submitted"] == sum(record["submitted_us"] is not None for record in records)
assert summary["presented"] == sum(record["presented_us"] is not None for record in records)
assert summary["dropped"] is None or summary["dropped"] == sum(record["discarded"] for record in records)
if summary["status"] == "pass":
    assert not headless and summary["presentation_supported"]
    assert summary["submitted"] == summary["presented"] == len(records)
    assert summary["presentation_ordered"] and summary["dropped"] == 0
    assert summary["queue_high_water"] <= 128
    for record in records:
        deadline = record["scheduled_us"] + 100_000
        assert record["accepted_us"] <= deadline
        assert record["submitted_us"] <= deadline
        assert record["presented_us"] <= deadline
    stamps = [record["presented_us"] for record in records]
    assert stamps == sorted(stamps)
if headless:
    assert summary["status"] == "unsupported"
fixtures = json.loads((Path(__file__).resolve().parent.parent / "evidence/generator-v1-golden.json").read_text())
matching = [item for item in fixtures if item["seed"] == metadata["seed"] and item["preset"] == metadata["preset"] and item["tick"] == summary["last_generated_tick"]]
if matching:
    assert summary["model_checksum"] == matching[0]["checksum"]
print(json.dumps({"report": str(args.report), "verified_ticks": len(records), "status": summary["status"], "checksum": summary["model_checksum"], "source_checksum": metadata["source_checksum"]}))
