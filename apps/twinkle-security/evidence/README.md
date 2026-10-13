# Release evidence

Collected 2026-10-13 on Linux x86_64 / Ryzen 9 5900X / RTX 4080 SUPER (Vulkan,
NVIDIA 595.99.02), fixed 1640 × 1020 logical window, scale 1. Native variants ran
sequentially. Every report uses engine commit
`ab1f554f5c0baf829e32d38567793ef4af8c259c` and compiled source checksum
`1855781b51f7a52f0fb016a8c1c061a4b35950991a0eff9bc38aa067a073009a`.

| Report | Preset | Accepted ticks | Displayed ticks | Presentation p95 (ms) | Strict rendered result |
| --- | --- | ---: | ---: | ---: | --- |
| [gpu-standard](gpu-standard.jsonl) | standard | 3000 | 3000 | 31.349 | fail |
| [cpu-smoke](cpu-smoke.jsonl) | standard | 100 | 100 | 30.247 | pass |
| [gpu-dense](gpu-dense.jsonl) | dense | 100 | 100 | 34.153 | pass |
| [gpu-stress](gpu-stress.jsonl) | stress | 100 | 100 | 44.377 | pass |
| [headless-standard](headless-standard.jsonl) | standard | 3000 | 0 | — | unsupported |
| [headless-stress](headless-stress.jsonl) | stress | 100 | 0 | — | unsupported |
| [headless-cpu-reference](headless-cpu-reference.jsonl) | standard | 100 | 0 | — | unsupported |

The full 3,000-tick GPU run displayed every revision in order and discarded none,
but six exceeded their 100-ms deadline. Its **fail** is preserved; short CPU/dense/
stress passes do not waive the full-run gate. Headless runs are unpaced preparation
throughput over the same simulated ticks and remain unsupported for rendering.
Their checksums match the corresponding native runs exactly.

[Acceptance audit](ACCEPTANCE.md) maps requirements to code, tests, reports and
visual proof. [Full GPU summary](gpu-standard-summary.json) includes actual adapter
identity, sampled memory ranges, cell dimensions and missed tick IDs. Large raw
pipeline traces and the pre-fix report/profile are preserved under ignored
`target/security-evidence/` in the project checkout.

[Golden vectors](generator-v1-golden.json) are an input to the generator tests and
must not be deleted or casually regenerated. Debug and release output match them.

The baseline, surge, credential, scanning, mitigation, wrap and narrow PNGs are
software renders of the real native UiHost declaration. They prove layout;
compositor timings come exclusively from native JSONL feedback.

```sh
python3 apps/twinkle-security/scripts/verify_report.py apps/twinkle-security/evidence/gpu-standard.jsonl
```
