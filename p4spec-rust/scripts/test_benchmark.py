import importlib.util
import json
from pathlib import Path
import stat
import tempfile
from types import SimpleNamespace
import unittest


SCRIPT = Path(__file__).with_name("benchmark.py")
SPEC = importlib.util.spec_from_file_location("benchmark", SCRIPT)
benchmark = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(benchmark)


class BenchmarkTest(unittest.TestCase):
    def test_collect_preflights_before_reserving_stage_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            rust_bin = root / "rust"
            ocaml_bin = root / "ocaml"
            ocaml_bin.write_text("#!/bin/sh\nprintf 'passed\\n'\n")
            ocaml_bin.chmod(ocaml_bin.stat().st_mode | stat.S_IXUSR)
            args = SimpleNamespace(
                repo=Path(__file__).parents[2],
                rust_bin=rust_bin,
                ocaml_bin=ocaml_bin,
                rust_commit="rust-commit",
                ocaml_commit="ocaml-commit",
                p4c_commit="p4c-commit",
                rustc_version="rustc exact",
                ocamlc_version="ocamlc exact",
                output_root=root / "output",
                stage="retryable",
                rust_cache_mode="absent",
                only=["rust"],
                warmups=0,
                measurements=1,
                relation="Program_inst",
                include=root,
                program=root / "input.p4",
            )
            stage = args.output_root / args.stage

            with self.assertRaises(FileNotFoundError):
                benchmark.collect(args)
            self.assertFalse(stage.exists())

            rust_bin.write_text("#!/bin/sh\nprintf 'passed\\n'\n")
            rust_bin.chmod(rust_bin.stat().st_mode | stat.S_IXUSR)
            benchmark.collect(args)
            self.assertTrue((stage / "summary.json").is_file())

            with self.assertRaises(FileExistsError):
                benchmark.collect(args)

    def test_parse_time_reads_wall_seconds_and_maximum_rss(self):
        sample = benchmark.parse_time(
            "        12.34 real         9.87 user         0.11 sys\n"
            "            456789  maximum resident set size\n"
        )

        self.assertEqual(sample, {"wall_seconds": 12.34, "max_rss_bytes": 456789})

    def test_parse_time_rejects_incomplete_output(self):
        with self.assertRaisesRegex(ValueError, "maximum resident set size"):
            benchmark.parse_time("1.23 real\n")

    def test_schedule_reverses_every_other_round(self):
        configs = ["rust", "ocaml-uncached", "ocaml-cached"]

        self.assertEqual(
            benchmark.schedule(configs, 3),
            [
                (1, "rust"),
                (1, "ocaml-uncached"),
                (1, "ocaml-cached"),
                (2, "ocaml-cached"),
                (2, "ocaml-uncached"),
                (2, "rust"),
                (3, "rust"),
                (3, "ocaml-uncached"),
                (3, "ocaml-cached"),
            ],
        )

    def test_select_configs_supports_rust_only_stage_runs(self):
        configs = [
            benchmark.Config("rust_cached", ["rust"]),
            benchmark.Config("ocaml_uncached", ["ocaml", "-no-cache"]),
            benchmark.Config("ocaml_cached", ["ocaml"]),
        ]

        selected = benchmark.select_configs(configs, ["rust"])

        self.assertEqual([config.name for config in selected], ["rust_cached"])

    def test_parser_records_explicit_build_toolchains(self):
        args = benchmark.parser().parse_args(
            [
                "--repo", ".",
                "--rust-bin", "rust",
                "--ocaml-bin", "ocaml",
                "--rust-commit", "rust-commit",
                "--ocaml-commit", "ocaml-commit",
                "--p4c-commit", "p4c-commit",
                "--rustc-version", "rustc exact",
                "--ocamlc-version", "ocamlc exact",
                "--output-root", "output",
                "--stage", "baseline",
            ]
        )

        self.assertEqual(args.rustc_version, "rustc exact")
        self.assertEqual(args.ocamlc_version, "ocamlc exact")

    def test_stats_reports_median_and_sample_variance(self):
        self.assertEqual(
            benchmark.stats([1.0, 2.0, 6.0]),
            {"median": 2.0, "sample_variance": 7.0, "samples": [1.0, 2.0, 6.0]},
        )

    def test_run_sample_preserves_failure_and_rejects_it(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fake_time = root / "time"
            fake_time.write_text(
                "#!/bin/sh\n"
                "out=\"$3\"\n"
                "shift 3\n"
                "\"$@\"\n"
                "status=$?\n"
                "printf '0.25 real\\n1024 maximum resident set size\\n' > \"$out\"\n"
                "exit $status\n"
            )
            fake_time.chmod(fake_time.stat().st_mode | stat.S_IXUSR)
            sample_dir = root / "sample"

            with self.assertRaisesRegex(benchmark.RunFailed, "exit status 7"):
                benchmark.run_sample(
                    ["/bin/sh", "-c", "printf nope; printf bad >&2; exit 7"],
                    sample_dir,
                    fake_time,
                )

            self.assertEqual((sample_dir / "stdout.txt").read_text(), "nope")
            self.assertEqual((sample_dir / "stderr.txt").read_text(), "bad")
            result = json.loads((sample_dir / "result.json").read_text())
            self.assertEqual(result["status"], 7)
            self.assertFalse(result["passed"])

    def test_run_sample_rejects_success_without_pass_marker(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            fake_time = root / "time"
            fake_time.write_text(
                "#!/bin/sh\n"
                "out=\"$3\"\n"
                "shift 3\n"
                "\"$@\"\n"
                "status=$?\n"
                "printf '0.25 real\\n1024 maximum resident set size\\n' > \"$out\"\n"
                "exit $status\n"
            )
            fake_time.chmod(fake_time.stat().st_mode | stat.S_IXUSR)

            with self.assertRaisesRegex(benchmark.RunFailed, "missing pass marker"):
                benchmark.run_sample(["/usr/bin/true"], root / "sample", fake_time)


if __name__ == "__main__":
    unittest.main()
