import subprocess
import sys
import shutil
import re
import tempfile
import os


def check_tool(name):
    return shutil.which(name) is not None


def run_tool(cmd, stdin_file=None, timeout=10):
    input_fd = open(stdin_file, "r") if stdin_file else None
    try:
        res = subprocess.run(
            cmd, stdin=input_fd, capture_output=True, text=True, timeout=timeout
        )
        return res
    except subprocess.TimeoutExpired:
        print("@@ARGO_ERR@@Profiler execution timed out (limit: 10s).")
        return None
    except Exception as e:
        print(f"@@ARGO_ERR@@Failed to execute {' '.join(cmd[:2])}: {e}")
        return None
    finally:
        if input_fd:
            input_fd.close()


def get_input_files(input_path):
    if not input_path:
        return []
    if os.path.isdir(input_path):
        files = []
        for f in os.listdir(input_path):
            if f.startswith("in_") and f.endswith(".txt"):
                files.append(os.path.join(input_path, f))
        files.sort(
            key=lambda x: [int(c) if c.isdigit() else c for c in re.split(r"(\d+)", x)]
        )
        return files
    elif os.path.exists(input_path):
        return [input_path]
    return []


def run_perf(binary, input_files):
    if not check_tool("perf"):
        print("@@ARGO_ERR@@'perf' is not installed or not in PATH.")
        return

    events = [
        "cycles",
        "instructions",
        "cache-references",
        "cache-misses",
        "branches",
        "branch-misses",
    ]

    print("@@ARGO_SECTION@@Hardware Performance Counters (perf)")

    stats = {}
    files_to_run = input_files if input_files else [None]

    for inp in files_to_run:
        cmd = ["perf", "stat", "-x;", "-e", ",".join(events), binary]
        res = run_tool(cmd, inp, timeout=10)
        if not res:
            continue

        if res.returncode != 0:
            if (
                "Permission denied" in res.stderr
                or "Access to performance monitoring" in res.stderr
            ):
                print(
                    "@@ARGO_ERR@@Permission denied for perf counters. Run with sudo or set kernel.perf_event_paranoid=-1."
                )
                return
            else:
                print(
                    f"@@ARGO_ERR@@perf failed on {inp} (exit code {res.returncode}): {res.stderr.strip()}"
                )
                continue

        for line in res.stderr.splitlines():
            if line.startswith("#"):
                continue
            parts = line.split(";")
            if len(parts) >= 3:
                val, event = parts[0].strip(), parts[2].strip()
                clean_event = event.split(":")[0]
                if (
                    val
                    and clean_event
                    and "not supported" not in val.lower()
                    and "<not counted>" not in val.lower()
                ):
                    try:
                        numeric_val = int(val.replace(",", "").replace(".", ""))
                        stats[clean_event] = stats.get(clean_event, 0) + numeric_val
                    except ValueError:
                        pass

    if not stats:
        print("@@ARGO_ERR@@No counter values collected.")
        return

    for event in events:
        if event in stats:
            print(f"@@ARGO_STAT@@{event} (cumulative)@@{stats[event]:,}")

    if "cycles" in stats and "instructions" in stats and stats["cycles"] > 0:
        ipc = stats["instructions"] / stats["cycles"]
        print(f"@@ARGO_STAT@@Calculated IPC@@{ipc:.2f} (Target: >1.50)")
    if "branches" in stats and "branch-misses" in stats and stats["branches"] > 0:
        branch_rate = (stats["branch-misses"] / stats["branches"]) * 100
        print(
            f"@@ARGO_STAT@@Branch Mispredict Rate@@{branch_rate:.2f}% (Target: <3.00%)"
        )
    if (
        "cache-references" in stats
        and "cache-misses" in stats
        and stats["cache-references"] > 0
    ):
        cache_rate = (stats["cache-misses"] / stats["cache-references"]) * 100
        print(f"@@ARGO_STAT@@L1 Cache Miss Rate@@{cache_rate:.2f}% (Target: <5.00%)")


def run_perf_sampling(binary, input_files):
    if not check_tool("perf"):
        return

    print("@@ARGO_SECTION@@Instruction-Level CPU Hotspots (perf sampling)")

    with tempfile.NamedTemporaryFile(suffix=".data", delete=False) as tmp_data:
        perf_data_path = tmp_data.name

    try:
        files_to_run = input_files if input_files else [None]
        first = True

        for inp in files_to_run:
            if first:
                record_cmd = [
                    "perf",
                    "record",
                    "-q",
                    "-e",
                    "cycles:u",
                    "-o",
                    perf_data_path,
                    "--",
                    binary,
                ]
            else:
                record_cmd = [
                    "perf",
                    "record",
                    "-q",
                    "-e",
                    "cycles:u",
                    "-o",
                    perf_data_path,
                    "--append",
                    "--",
                    binary,
                ]

            res = run_tool(record_cmd, inp, timeout=12)
            if not res or res.returncode != 0:
                if first:
                    record_cmd = [
                        "perf",
                        "record",
                        "-q",
                        "-e",
                        "cycles",
                        "-o",
                        perf_data_path,
                        "--",
                        binary,
                    ]
                else:
                    record_cmd = [
                        "perf",
                        "record",
                        "-q",
                        "-e",
                        "cycles",
                        "-o",
                        perf_data_path,
                        "--append",
                        "--",
                        binary,
                    ]
                res = run_tool(record_cmd, inp, timeout=12)

            if res and res.returncode == 0:
                first = False

        if first:  
            print(
                "@@ARGO_ERR@@perf record failed. Hotspot sampling is restricted on this environment."
            )
            return

        report_cmd = [
            "perf",
            "report",
            "-i",
            perf_data_path,
            "--stdio",
            "--no-children",
            "-n",
        ]
        report_res = subprocess.run(
            report_cmd, capture_output=True, text=True, timeout=5
        )

        if report_res.returncode == 0:
            print("@@ARGO_INFO@@Top Function Hotspots:")
            printed_symbols = 0
            for line in report_res.stdout.splitlines():
                if line.strip().startswith("#"):
                    continue
                collapsed = re.sub(r"\s+", " ", line.strip())
                parts = collapsed.split()
                if parts and "%" in parts[0]:
                    try:
                        pct = float(parts[0].replace("%", ""))
                        if pct > 0.5:
                            print(f"@@ARGO_INFO@@  {collapsed}")
                            printed_symbols += 1
                            if printed_symbols >= 5:
                                break
                    except ValueError:
                        pass
            if printed_symbols == 0:
                print("@@ARGO_INFO@@  No distinct function hotspots captured.")

        annotate_cmd = [
            "perf",
            "annotate",
            "-i",
            perf_data_path,
            "--stdio",
            "--no-source",
        ]
        annotate_res = subprocess.run(
            annotate_cmd, capture_output=True, text=True, timeout=5
        )

        if annotate_res.returncode == 0:
            print("@@ARGO_INFO@@")
            print("@@ARGO_INFO@@Top Instruction-Level Assembly Hotspots:")

            hot_instructions = []
            for line in annotate_res.stdout.splitlines():
                parts = line.split(":")
                if len(parts) >= 3:
                    pct_str = parts[0].strip()
                    addr = parts[1].strip()
                    insn = ":".join(parts[2:]).strip()

                    try:
                        pct = float(pct_str)
                        if pct > 1.0:
                            hot_instructions.append((pct, addr, insn))
                    except ValueError:
                        pass

            hot_instructions.sort(key=lambda x: x[0], reverse=True)

            printed_insns = 0
            for pct, addr, insn in hot_instructions:
                clean_insn = re.sub(r"\s+", " ", insn)
                print(f"@@ARGO_INFO@@  {pct:5.2f}% at 0x{addr}: {clean_insn}")
                printed_insns += 1
                if printed_insns >= 5:
                    break
            if printed_insns == 0:
                print(
                    "@@ARGO_INFO@@  No single instruction exceeded the 1.0% threshold."
                )

    except Exception as e:
        print(f"@@ARGO_ERR@@Failed to run micro-sampling profile: {e}")
    finally:
        if os.path.exists(perf_data_path):
            try:
                os.remove(perf_data_path)
            except OSError:
                pass


def run_valgrind(binary, input_files):
    if not check_tool("valgrind"):
        print("@@ARGO_ERR@@'valgrind' is not installed or not in PATH.")
        return

    print("@@ARGO_SECTION@@Cache Locality Estimates (Cachegrind)")

    total_metrics = {}
    files_to_run = input_files if input_files else [None]

    for inp in files_to_run:
        with tempfile.NamedTemporaryFile() as tmp:
            cmd = [
                "valgrind",
                "--tool=cachegrind",
                f"--cachegrind-out-file={tmp.name}",
                binary,
            ]
            res = run_tool(cmd, inp, timeout=15)
            if not res or res.returncode != 0:
                continue

            for line in res.stderr.splitlines():
                clean = re.sub(r"==\d+==\s*", "", line).strip()
                collapsed = re.sub(r"\s+", " ", clean)

                if ":" in collapsed:
                    parts = collapsed.split(":", 1)
                    key = parts[0].strip()
                    val_str = parts[1].strip()

                    num_match = re.search(r"([\d,]+)", val_str)
                    if num_match:
                        try:
                            val = int(num_match.group(1).replace(",", ""))
                            total_metrics[key] = total_metrics.get(key, 0) + val
                        except ValueError:
                            pass

    if not total_metrics:
        print("@@ARGO_ERR@@No cache metrics captured.")
        return

    for key, val in total_metrics.items():
        if any(kw in key for kw in ["refs", "misses"]):
            print(f"@@ARGO_INFO@@{key} (cumulative): {val:,}")

    def get_metric(pattern):
        for k, v in total_metrics.items():
            if re.search(pattern, k):
                return v
        return 0

    i_refs = get_metric(r"^I\s+refs")
    i1_misses = get_metric(r"^I1\s+misses")
    d_refs = get_metric(r"^D\s+refs")
    d1_misses = get_metric(r"^D1\s+misses")

    ll_refs = get_metric(r"^(LL|L2|L3)\s+refs")
    ll_misses = get_metric(r"^(LL|L2|L3)\s+misses")

    if i_refs > 0 and i1_misses > 0:
        rate = (i1_misses / i_refs) * 100
        print(f"@@ARGO_INFO@@I1 miss rate: {rate:.2f}%")
    if d_refs > 0 and d1_misses > 0:
        rate = (d1_misses / d_refs) * 100
        print(f"@@ARGO_INFO@@D1 miss rate: {rate:.2f}%")
    if ll_refs > 0 and ll_misses > 0:
        rate = (ll_misses / ll_refs) * 100
        print(f"@@ARGO_INFO@@Last-level miss rate: {rate:.2f}%")


def run_mca(asm_file):
    if not check_tool("llvm-mca"):
        print("@@ARGO_ERR@@'llvm-mca' is not installed or not in PATH.")
        return

    if not asm_file or not os.path.exists(asm_file):
        print("@@ARGO_ERR@@No valid assembly file found for llvm-mca.")
        return

    print("@@ARGO_SECTION@@Static Microarchitectural Analysis (llvm-mca)")

    cmd = ["llvm-mca", asm_file]
    res = run_tool(cmd, timeout=10)
    if not res:
        return

    if res.returncode != 0:
        print(
            f"@@ARGO_ERR@@llvm-mca failed (exit code {res.returncode}): {res.stderr.strip()}"
        )
        return

    for line in res.stdout.splitlines():
        stripped = line.strip()
        if any(
            stripped.startswith(prefix)
            for prefix in ["Total Cycles:", "Total uOps:", "IPC:", "Block RThroughput:"]
        ):
            parts = stripped.split(":", 1)
            if len(parts) == 2:
                print(f"@@ARGO_STAT@@{parts[0].strip()}@@{parts[1].strip()}")
            else:
                print(f"@@ARGO_INFO@@{stripped}")


def run_vectorization_report(compiler, source_file):
    if not compiler or not source_file or not os.path.exists(source_file):
        return

    print("@@ARGO_SECTION@@Compiler Vectorization Diagnostics")

    compiler_parts = compiler.split()
    base_compiler = compiler_parts[0]
    is_clang = "clang" in base_compiler.lower()

    cmd = list(compiler_parts)
    if is_clang:
        cmd.extend(
            ["-O2", "-Rpass-missed=loop-vectorize", "-c", source_file, "-o", os.devnull]
        )
    else:
        cmd.extend(
            [
                "-O2",
                "-ftree-vectorize",
                "-fopt-info-vec-missed",
                "-c",
                source_file,
                "-o",
                os.devnull,
            ]
        )

    try:
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        report = res.stderr

        issues = 0
        for line in report.splitlines():
            if any(
                kw in line.lower()
                for kw in ["missed", "not vectorized", "failed", "remark:"]
            ):
                clean_line = line.strip()
                if source_file in clean_line:
                    clean_line = clean_line.split(source_file)[-1].strip(" :")
                print(f"@@ARGO_INFO@@{clean_line}")
                issues += 1
                if issues >= 10:
                    print("@@ARGO_INFO@@... (diagnostic report truncated)")
                    break
        if issues == 0:
            print(
                "@@ARGO_STAT@@Vectorization Blocks@@Passed (Clean SIMD auto-vectorization)"
            )
    except Exception as e:
        print(f"@@ARGO_ERR@@Failed to run vectorization diagnostics: {e}")


def run_size_analysis(binary):
    if not check_tool("size"):
        return

    print("@@ARGO_SECTION@@Binary Memory Footprint")
    cmd = ["size", binary]
    res = run_tool(cmd, timeout=5)
    if not res or res.returncode != 0:
        return

    lines = res.stdout.splitlines()
    if len(lines) >= 2:
        headers = lines[0].split()
        values = lines[1].split()
        stats = dict(zip(headers, values))

        if "text" in stats:
            print(
                f"@@ARGO_STAT@@Code (.text) segment@@{int(stats['text']):,} bytes (Instructions)"
            )
        if "data" in stats:
            print(
                f"@@ARGO_STAT@@Data (.data) segment@@{int(stats['data']):,} bytes (Initialized globals)"
            )
        if "bss" in stats:
            bss_bytes = int(stats["bss"])
            bss_mb = bss_bytes / (1024 * 1024)
            print(
                f"@@ARGO_STAT@@BSS (.bss) segment@@{bss_bytes:,} bytes ({bss_mb:.2f} MB globals)"
            )


def main():
    if len(sys.argv) < 6:
        print("@@ARGO_ERR@@Insufficient arguments supplied to profiler script.")
        return

    binary = sys.argv[1]
    input_path = sys.argv[2] if sys.argv[2] != "none" else None
    asm_file = sys.argv[3] if sys.argv[3] != "none" else None
    source_file = sys.argv[4] if sys.argv[4] != "none" else None
    compiler = sys.argv[5] if sys.argv[5] != "none" else None

    input_files = get_input_files(input_path)

    run_perf(binary, input_files)
    run_perf_sampling(binary, input_files)
    run_valgrind(binary, input_files)
    run_mca(asm_file)
    run_vectorization_report(compiler, source_file)
    run_size_analysis(binary)


if __name__ == "__main__":
    main()
