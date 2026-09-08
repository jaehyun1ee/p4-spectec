#!/usr/bin/env python3
"""Collect reproducible whole-process p4spec CLI benchmark samples."""

import argparse
import hashlib
import json
import platform
import re
import statistics
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


PASS_MARKER = "passed"


class RunFailed(RuntimeError):
    pass


@dataclass(frozen=True)
class Config:
    name: str
    command: list[str]


def parse_time(output):
    wall_match = re.search(r"^\s*([0-9.]+)\s+real\b", output, re.MULTILINE)
    rss_match = re.search(
        r"^\s*(\d+)\s+maximum resident set size\b", output, re.MULTILINE
    )
    if wall_match is None:
        raise ValueError("time output lacks real wall time")
    if rss_match is None:
        raise ValueError("time output lacks maximum resident set size")
    return {
        "wall_seconds": float(wall_match.group(1)),
        "max_rss_bytes": int(rss_match.group(1)),
    }


def schedule(configs, rounds):
    result = []
    for round_number in range(1, rounds + 1):
        ordered = configs if round_number % 2 else reversed(configs)
        result.extend((round_number, config) for config in ordered)
    return result


def select_configs(configs, implementations):
    if not implementations:
        return configs
    prefixes = {
        "rust": ("rust_",),
        "ocaml-uncached": ("ocaml_uncached",),
        "ocaml-cached": ("ocaml_cached",),
    }
    selected_prefixes = tuple(
        prefix for implementation in implementations for prefix in prefixes[implementation]
    )
    return [config for config in configs if config.name.startswith(selected_prefixes)]


def stats(samples):
    return {
        "median": statistics.median(samples),
        "sample_variance": statistics.variance(samples) if len(samples) > 1 else 0.0,
        "samples": samples,
    }


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def run_sample(command, sample_dir, time_binary=Path("/usr/bin/time")):
    sample_dir.mkdir(parents=True, exist_ok=False)
    stdout_path = sample_dir / "stdout.txt"
    stderr_path = sample_dir / "stderr.txt"
    timing_path = sample_dir / "time.txt"
    write_json(sample_dir / "command.json", command)
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        completed = subprocess.run(
            [str(time_binary), "-l", "-o", str(timing_path), *command],
            stdout=stdout,
            stderr=stderr,
            check=False,
        )
    stdout_text = stdout_path.read_text(errors="replace")
    passed = completed.returncode == 0 and PASS_MARKER in stdout_text.splitlines()
    result = {"status": completed.returncode, "passed": passed}
    if timing_path.exists():
        result.update(parse_time(timing_path.read_text()))
    write_json(sample_dir / "result.json", result)
    if completed.returncode != 0:
        raise RunFailed(f"command failed with exit status {completed.returncode}")
    if not passed:
        raise RunFailed(f"command output is missing pass marker {PASS_MARKER!r}")
    return result


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def command_output(command, cwd=None):
    return subprocess.run(
        command, cwd=cwd, text=True, capture_output=True, check=True
    ).stdout.strip()


def host_metadata():
    metadata = {
        "machine": platform.machine(),
        "os": platform.platform(),
        "python": platform.python_version(),
    }
    if platform.system() == "Darwin":
        hardware = command_output(
            [
                "system_profiler",
                "SPHardwareDataType",
                "-json",
            ]
        )
        item = json.loads(hardware)["SPHardwareDataType"][0]
        for source, target in [
            ("machine_model", "model"),
            ("chip_type", "cpu"),
            ("physical_memory", "ram"),
        ]:
            if source in item:
                metadata[target] = item[source]
    return metadata


def common_args(repo, relation, program, include):
    return [str(repo / "spec"), "--rel", relation, "-i", str(include), "-p", str(program)]


def configs(args):
    repo = args.repo.resolve()
    common = common_args(repo, args.relation, args.program.resolve(), args.include.resolve())
    rust_base = [str(args.rust_bin.resolve()), "run", "--al", *common]
    if args.rust_cache_mode == "off":
        rust_base.append("--no-cache")
    ocaml_base = [str(args.ocaml_bin.resolve()), "run", str(repo / "spec"), "-al"]
    ocaml_tail = [
        "-rel",
        args.relation,
        "-i",
        str(args.include.resolve()),
        "-p",
        str(args.program.resolve()),
    ]
    rust_label = "rust_uncached" if args.rust_cache_mode in ("absent", "off") else "rust_cached"
    return [
        Config(rust_label, rust_base),
        Config("rust_uncached_det" if rust_label.endswith("uncached") else "rust_cached_det", [*rust_base, "--det"]),
        Config("ocaml_uncached", [*ocaml_base, *ocaml_tail, "-no-cache"]),
        Config("ocaml_uncached_det", [*ocaml_base, *ocaml_tail, "-no-cache", "-det"]),
        Config("ocaml_cached", [*ocaml_base, *ocaml_tail]),
        Config("ocaml_cached_det", [*ocaml_base, *ocaml_tail, "-det"]),
    ]


def collect(args):
    output_dir = args.output_root.resolve() / args.stage
    output_dir.mkdir(parents=True, exist_ok=False)
    config_list = select_configs(configs(args), args.only)
    metadata = {
        "created_at": datetime.now(timezone.utc).isoformat(),
        "stage": args.stage,
        "repo": str(args.repo.resolve()),
        "source_commit": command_output(["git", "rev-parse", "HEAD"], args.repo),
        "rust_build_commit": args.rust_commit,
        "ocaml_build_commit": args.ocaml_commit,
        "p4c_revision": args.p4c_commit,
        "spec_path": str((args.repo / "spec").resolve()),
        "include_path": str(args.include.resolve()),
        "program_path": str(args.program.resolve()),
        "relation": args.relation,
        "warmups_per_config": args.warmups,
        "measurements_per_config": args.measurements,
        "rust_cache_mode": args.rust_cache_mode,
        "rust_binary": str(args.rust_bin.resolve()),
        "rust_binary_sha256": sha256(args.rust_bin.resolve()),
        "ocaml_binary": str(args.ocaml_bin.resolve()),
        "ocaml_binary_sha256": sha256(args.ocaml_bin.resolve()),
        "rustc": args.rustc_version,
        "ocamlc": args.ocamlc_version,
        "host": host_metadata(),
        "configs": {config.name: config.command for config in config_list},
    }
    write_json(output_dir / "metadata.json", metadata)

    by_name = {config.name: config for config in config_list}
    for phase, rounds in [("warmup", args.warmups), ("measured", args.measurements)]:
        for round_number, name in schedule(list(by_name), rounds):
            sample_dir = output_dir / phase / f"round-{round_number:02d}" / name
            print(f"{phase} round {round_number}: {name}", flush=True)
            try:
                run_sample(by_name[name].command, sample_dir)
            except Exception as error:
                write_json(
                    output_dir / "failure.json",
                    {"phase": phase, "round": round_number, "config": name, "error": str(error)},
                )
                raise

    summary = {}
    for name in by_name:
        results = []
        for round_number in range(1, args.measurements + 1):
            path = output_dir / "measured" / f"round-{round_number:02d}" / name / "result.json"
            results.append(json.loads(path.read_text()))
        summary[name] = {
            "wall_seconds": stats([result["wall_seconds"] for result in results]),
            "max_rss_bytes": stats([result["max_rss_bytes"] for result in results]),
        }
    write_json(output_dir / "summary.json", summary)
    return output_dir


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--repo", type=Path, required=True)
    result.add_argument("--rust-bin", type=Path, required=True)
    result.add_argument("--ocaml-bin", type=Path, required=True)
    result.add_argument("--rust-commit", required=True)
    result.add_argument("--ocaml-commit", required=True)
    result.add_argument("--p4c-commit", required=True)
    result.add_argument("--rustc-version", required=True)
    result.add_argument("--ocamlc-version", required=True)
    result.add_argument("--output-root", type=Path, required=True)
    result.add_argument("--stage", required=True)
    result.add_argument("--rust-cache-mode", choices=["absent", "off", "on"], default="absent")
    result.add_argument(
        "--only",
        action="append",
        choices=["rust", "ocaml-uncached", "ocaml-cached"],
        help="limit collection to an implementation group (repeatable)",
    )
    result.add_argument("--warmups", type=int, default=2)
    result.add_argument("--measurements", type=int, default=10)
    result.add_argument("--relation", default="Program_inst")
    result.add_argument("--include", type=Path)
    result.add_argument("--program", type=Path)
    return result


def main(argv=None):
    args = parser().parse_args(argv)
    if args.warmups < 0 or args.measurements < 1:
        parser().error("warmups must be nonnegative and measurements must be positive")
    if args.include is None:
        args.include = args.repo / "p4c/p4include"
    if args.program is None:
        args.program = args.repo / "p4c/testdata/p4_16_samples/xor_test.p4"
    try:
        output_dir = collect(args)
    except (OSError, subprocess.CalledProcessError, ValueError, RunFailed) as error:
        print(f"benchmark failed: {error}", file=sys.stderr)
        return 1
    print(output_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
