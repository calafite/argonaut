# Argonaut

Argonaut (`argo`) is a CLI toolkit for C++ competitive programming. 

It was built to solve a specific annoyance: online judges require single-file submissions, which forces competitive programmers to use massive boilerplate templates or manually copy-paste algorithms during contests. Argonaut lets you write modular C++ with normal `#include` directives to a local library, and handles the dependency resolution, AST-based dead-code elimination, and minification before submission.

Beyond bundling, it uses Linux system primitives to safely sandbox local test executions and wraps various profilers to analyze microarchitectural bottlenecks.

## Overview

* **AST-based Bundling:** Uses Tree-sitter C++ grammars to parse local `#include` headers, trace the symbols used by `main()`, and tree-shake away unused structs, templates, and functions.
* **Kernel Sandboxing:** Local test execution is wrapped in `libseccomp` BPF filters to block network sockets, process cloning, and filesystem writes, preventing runaway code or fork bombs from freezing your machine.
* **Crash Tracing:** If a test case triggers a `SIGSEGV` or `SIGFPE`, Argonaut intercepts it and runs an embedded GDB Python script to print the exact offending source line automatically.
* **Hardware Profiling:** Wraps Linux `perf`, `valgrind`, and `llvm-mca` to report instructions per cycle (IPC), branch misprediction rates, and L1 cache misses.
* **Browser Ingestion:** Runs a local HTTP daemon that receives problem limits and test cases directly from the Competitive Companion browser extension.

## Dependencies

Argonaut is written in Rust, but relies heavily on Linux system utilities for its tracing and profiling features.

```bash
# Ubuntu/Debian
sudo apt install g++ gdb libseccomp-dev linux-tools-common linux-tools-generic valgrind llvm clang-format python3

# Fedora
sudo dnf install gcc-c++ gdb libseccomp-devel perf valgrind llvm clang-tools-extra python3
```
*Note: Seccomp BPF sandboxing and `perf` profiling are exclusively supported on Linux.*

## Installation

Clone the repository and build using Cargo:

```bash
git clone https://github.com/YOUR_USERNAME/argonaut.git
cd argonaut
cargo install --path .
```

## Workflow

**1. Ingest a problem**  
Run the listener and click the Competitive Companion extension on a Codeforces/AtCoder problem page. Argonaut creates the directory, scaffolds the C++ file, and saves the test cases.
```bash
argo listen
```

**2. Build the solution**  
Compile with standard release optimizations (`-O2`), or use `debug` to compile with AddressSanitizer and UndefinedBehaviorSanitizer.
```bash
argo build A.cpp
argo debug A.cpp
```

**3. Run sandboxed tests**  
Executes the binary against the ingested test cases. Enforces the memory limit parsed from the source file header.
```bash
argo test
```

**4. Bundle for submission**  
Parses the AST, tree-shakes dead code from your included library headers, and emits a single file ready for the online judge. The `-M` flag applies macro-based minification to shorten identifiers.
```bash
argo bundle A.cpp -o submit.cpp -M
```

## Command Reference

| Command | Description |
| :--- | :--- |
| `argo listen` | Starts the HTTP daemon to catch payloads from Competitive Companion. |
| `argo new` | Manually scaffolds a new problem directory and template. |
| `argo build` | Compiles using GCC/Clang with standard warnings. |
| `argo debug` | Compiles with `-g`, ASan, and UBSan. |
| `argo test` | Runs the local test suite under the seccomp sandbox. |
| `argo bundle` | Runs the Tree-sitter AST dead-code elimination and bundler. |
| `argo format` | Auto-formats code with a CP-optimized `clang-format` profile. |
| `argo peek` | Inspects emitted assembly. Supports `--reduced` to cross-compile to RISC-V 64. |
| `argo profile` | Runs hardware PMU counters, cachegrind, and static pipeline analysis. |

## Project Layout

Argonaut creates a hidden `.argo/` directory in the problem root to keep the workspace clean. Build artifacts, test cases, and generated trace scripts are stored here.

```text
├── A.cpp                  
├── include/               
└── .argo/                 
    ├── A.out              
    ├── A.errors           
    ├── tracer.py          
    ├── profiler.py        
    └── tests/
        └── A/
            ├── in_1.txt   
            └── out_1.txt
```

## Configuration

Argonaut reads its configuration from a `Config.toml` file located in your system's default config directory (e.g., `~/.config/argo/Config.toml` on Linux).

```toml
[scaffold]
template_path = "~/cp/template.cpp"
short_name = true

[build]
compiler = "g++"
std = 20
include_dirs = ["~/cp/library"]
log_file = true
```
