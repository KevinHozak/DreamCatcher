import json
from pathlib import Path

from core.face_benchmark import execute_full_suite, evaluate_false_positive_tendencies, assert_zero_network


def test_face_runtime_benchmark_zero_network_execution():
    """Verify that candidate local face runtime runs with zero network activity."""
    with assert_zero_network():
        results = execute_full_suite()

    assert "benchmarks" in results
    assert len(results["benchmarks"]) >= 3
    for b in results["benchmarks"]:
        assert b["zero_network_verified"] is True
        assert b["warm_avg_ms"] > 0
        assert b["ram_peak_mb"] > 0


def test_false_positive_analysis_coverage():
    """Verify that false-positive and edge-case tendencies are thoroughly analyzed."""
    evals = evaluate_false_positive_tendencies()
    assert "BlazeFace-Lite" in evals
    assert "SCRFD-0.5G" in evals
    assert "MobileFaceNet-Embed" in evals
    assert "false_positive_rate_estimate" in evals["SCRFD-0.5G"]
