# Graph Report - proxy  (2026-11-22)

## Corpus Check
- Corpus is ~6,791 words - fits in a single context window. You may not need a graph.

## Summary
- 212 nodes · 412 edges · 8 communities
- Extraction: 95% EXTRACTED · 5% INFERRED · 0% AMBIGUOUS · INFERRED: 20 edges (avg confidence: 0.88)
- Ekstraksi AST berjalan lokal. Graphify tidak mengukur penggunaan token agen Codex untuk ekstraksi dokumen.

## Community Hubs (Navigation)
- Probe dan penghitung kesehatan
- Tes kesiapan HTTP
- Panduan operasi dan arsitektur
- Rotasi backend sehat
- Konfigurasi dan akses IP
- Penanganan request HTTP
- Tes forwarding dan pemulihan
- Dependensi antar-crate

## God Nodes (most connected - your core abstractions)
1. `RoundRobinPool` - 20 edges
2. `start_proxy()` - 16 edges
3. `Origin` - 15 edges
4. `CountedHealthCheck` - 14 edges
5. `ReverseProxy` - 14 edges
6. `Proxy` - 13 edges
7. `ProbeStreaks` - 11 edges
8. `CompletedProbe` - 10 edges
9. `IpAccessPolicy` - 10 edges
10. `HttpEndpointChecks` - 9 edges

## Surprising Connections (you probably didn't know these)
- `Rentetan hasil probe per backend` --references--> `CountedHealthCheck`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/health.rs
- `Probe kesiapan HTTP` --references--> `HttpHealthOptions`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/health.rs
- `Hasil probe TCP dan HTTP` --references--> `ProbeResult`  [INFERRED]
  README.md → crates/proxy-balancer/src/health.rs
- `Rentetan hasil probe per backend` --references--> `ProbeStreaks`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/health.rs
- `probe_json()` --implements--> `last_probe — probe terakhir yang selesai`  [INFERRED]
  crates/proxy-http/src/lib.rs → README.md

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Pembacaan kesiapan, penghitung dan probe terakhir untuk status backend** — readme_status_backend, readme_round_robin, readme_rentetan_probe, crates_proxy_balancer_src_lib_roundrobinpool_backend_probe_statuses, readme_last_probe [EXTRACTED 1.00]

## Communities (8 total, 0 thin omitted)

### Community 0 - "Probe dan penghitung kesehatan"
Cohesion: 0.09
Nodes (32): CompletedProbe, CountedHealthCheck, HttpEndpointChecks, HttpHealthOptions, ProbeResult, ProbeState, ProbeStreaks, Arc (+24 more)

### Community 1 - "Tes kesiapan HTTP"
Cohesion: 0.13
Nodes (31): accepts_ready_response_within_extended_timeout(), applies_consecutive_thresholds_to_ready_probes(), assert_probe(), backend_statuses(), denies_status_to_a_blocked_client(), excludes_http_500_and_rejoins_after_http_200(), excludes_timed_out_ready_endpoint_and_rejoins_after_recovery(), fetch() (+23 more)

### Community 2 - "Panduan operasi dan arsitektur"
Cohesion: 0.10
Nodes (24): Panduan agen, Pembaruan graf bersama melalui Git, graphify, Arsitektur workspace, Menempatkan perubahan, Mengembangkan struktur, Memeriksa perubahan, Menempatkan tes (+16 more)

### Community 3 - "Rotasi backend sehat"
Cohesion: 0.12
Nodes (21): HealthCheckMode, BackendStatus, RoundRobinPool, Arc, Backend, Backends, Box, Duration (+13 more)

### Community 4 - "Konfigurasi dan akses IP"
Cohesion: 0.25
Nodes (8): Args, IpAddr, NonZeroU32, NonZeroUsize, Option, SocketAddr, String, Vec

### Community 5 - "Penanganan request HTTP"
Cohesion: 0.09
Nodes (24): main(), Box, Error, Result, RequestBackend, ReverseProxy, Box, Error (+16 more)

### Community 6 - "Tes forwarding dan pemulihan"
Cohesion: 0.15
Nodes (16): alternates_backends_per_request_on_one_connection(), blocks_denied_and_unlisted_peers_before_forwarding(), fetch_marker(), forwards_post_and_returns_backend_response(), read_marker(), returns_bad_gateway_when_backend_is_offline(), routes_to_survivor_and_resumes_round_robin_after_recovery(), Server (+8 more)

### Community 7 - "Dependensi antar-crate"
Cohesion: 0.83
Nodes (4): proxy, proxy-balancer, proxy-http, proxy-security

## Knowledge Gaps
- **11 isolated node(s):** `Menempatkan perubahan`, `Mengembangkan struktur`, `Memeriksa perubahan`, `Menempatkan tes`, `Menulis kode Rust` (+6 more)
  These have ≤1 connection - possible missing edges or undocumented components.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `RoundRobinPool` connect `Rotasi backend sehat` to `Probe dan penghitung kesehatan`, `Panduan operasi dan arsitektur`, `Penanganan request HTTP`?**
  _High betweenness centrality (0.229) - this node is a cross-community bridge._
- **Why does `ReverseProxy` connect `Penanganan request HTTP` to `Panduan operasi dan arsitektur`, `Rotasi backend sehat`?**
  _High betweenness centrality (0.186) - this node is a cross-community bridge._
- **Why does `Pengujian kesiapan melalui listener` connect `Tes kesiapan HTTP` to `Panduan operasi dan arsitektur`?**
  _High betweenness centrality (0.152) - this node is a cross-community bridge._
- **Are the 12 inferred relationships involving `start_proxy()` (e.g. with `alternates_backends_per_request_on_one_connection()` and `blocks_denied_and_unlisted_peers_before_forwarding()`) actually correct?**
  _`start_proxy()` has 12 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Menempatkan perubahan`, `Mengembangkan struktur`, `Memeriksa perubahan` to the rest of the system?**
  _11 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Probe dan penghitung kesehatan` be split into smaller, more focused modules?**
  _Cohesion score 0.08695652173913043 - nodes in this community are weakly interconnected._
- **Should `Tes kesiapan HTTP` be split into smaller, more focused modules?**
  _Cohesion score 0.1253968253968254 - nodes in this community are weakly interconnected._
## Cakupan ekstraksi

- Manifest workspace di root (`Cargo.toml`) tidak menghasilkan simpul. Keempat manifest package tercakup.
