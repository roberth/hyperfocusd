# `hyperfocusd` 🔕

_Benchmark environment switch_

Hyperfocusd is a small daemon written in Rust for the purpose of reducing noise in benchmarks.

Specifically, its purpose is to abstract away relevant system configuration so that:
- Switching is secure and standardized
- System configuration tweaks are easy to share and collaborate on
- Toil is avoided

## Basic workflow

A typical usage consists of the following steps:

1. User or CI initiates a benchmark
2. `hyperfocus-on -- some-command --some-flags` is called
3. `hyperfocus-on` sends a message to the `hyperfocusd` socket and waits for a response
4. `hyperfocusd` configures the system for benchmarking
5. `hyperfocus-on` receives the response and starts `some-command --some-flags` as a child process. Stdio is forwarded.
6. `hyperfocus-on` waits for the child process to finish and returns the same exit status
7. `hyperfocusd` configures the system for not benchmarking

## Core functionality

Hyperfocusd takes care of:

- setting a fixed CPU frequency,
- configuring CPU power management,
- disabling background services,
- running one hyperfocus session at a time.

Hyperfocusd does not take care of:

- Environment warm-up
  - You can perform warm-up in the script that runs in your hyperfocus session.
- Statistics, monitoring, reporting
  - Any activities during the benchmark run the risk of creating noise.

## Details

### `HYPERFOCUSING` environment variable

`hyperfocus-on` sets `HYPERFOCUSING=1` so that the child process knows its operating environment is configured to be quiet.

### NixOS module

The hyperfocusd NixOS module adds a NixOS Specialisation that disables unwanted background services.

### Robust against user crashes

In case anything unexpected happens to the `hyperfocus-on` process, it may not be able to release its "lock" on the hyperfocus session. For example, it might perform an immediate process abort, or get OOM-killed.
As a fallback to the explicit release message, `hyperfocus` can passively watch the client connection and receive a signal when the other end is closed, e.g. due to the client process disappearing.
When this happens, it goes out of hyperfocus and logs the error.

To monitor `hyperfocusd` reliably, make sure your log monitoring solution can resume processing of journal entries from (perhaps shortly) before monitoring process start.

## Comparison with existing tools

Several Linux tools provide partially overlapping functionality for benchmark environment tuning:

### TuneD + tuned-adm

[TuneD](https://github.com/redhat-performance/tuned) is Red Hat's dynamic tuning daemon that offers profiles like "throughput-performance" and "latency-performance" to configure CPU governors and other system settings.

**Limitations:**
- Requires manual profile switching before and after benchmarks
- No automatic revert when benchmark completes
- No crash recovery mechanism

### cpupower

[cpupower](https://wiki.archlinux.org/title/CPU_frequency_scaling) is a kernel utility for CPU frequency and governor management. It can set the performance governor and disable turbo boost.

**Limitations:**
- Manual invocation only
- No session management
- Settings persist until manually reverted

### Custom wrapper scripts

Many people write bash scripts that configure CPU isolation (isolcpus, cpuset cgroups), set performance governors, run benchmarks, and clean up afterward.

While effective for their specific use cases, these scripts tend to be ad-hoc and non-standardized, leading to each team reinventing similar solutions. They may also lack robust error handling for edge cases like process crashes.

### What hyperfocusd adds

- **Automatic session management**: Daemon handles configuration enter/exit automatically around command execution
- **Crash recovery**: Socket-based monitoring ensures cleanup even if the client process crashes or gets OOM-killed
- **Standardized interface**: Teams can share and collaborate on system tuning configurations
- **Mutual exclusion**: Enforces "one session at a time" coordination across the system
- **NixOS integration**: Declarative configuration through the NixOS module system
