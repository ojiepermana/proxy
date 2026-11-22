# Graph Report - proxy  (2026-11-22)

## Corpus Check
- Corpus is ~6,365 words - fits in a single context window. You may not need a graph.

## Summary
- 198 nodes · 370 edges · 8 communities
- Extraction: 96% EXTRACTED · 4% INFERRED · 0% AMBIGUOUS · INFERRED: 14 edges (avg confidence: 0.85)
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
1. `RoundRobinPool` - 21 edges
2. `start_proxy()` - 16 edges
3. `Origin` - 15 edges
4. `ReverseProxy` - 14 edges
5. `Proxy` - 13 edges
6. `CountedHealthCheck` - 12 edges
7. `ProbeStreaks` - 10 edges
8. `IpAccessPolicy` - 10 edges
9. `Args` - 9 edges
10. `HttpHealthOptions` - 9 edges

## Surprising Connections (you probably didn't know these)
- `Probe kesiapan HTTP` --references--> `HttpHealthOptions`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/health.rs
- `Rentetan hasil probe per backend` --references--> `ProbeStreaks`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/health.rs
- `Rentetan hasil probe per backend` --references--> `CountedHealthCheck`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/health.rs
- `Round robin per request` --references--> `RoundRobinPool`  [EXTRACTED]
  README.md → crates/proxy-balancer/src/lib.rs
- `Arsitektur workspace` --references--> `ReverseProxy`  [EXTRACTED]
  docs/architecture.md → crates/proxy-http/src/lib.rs

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Pembacaan kesiapan dan penghitung untuk status backend** — readme_status_backend, readme_round_robin, readme_rentetan_probe, crates_proxy_balancer_src_lib_roundrobinpool_backend_probe_statuses [EXTRACTED 1.00]

## Communities (8 total, 0 thin omitted)

### Community 0 - "Probe dan penghitung kesehatan"
Cohesion: 0.10
Nodes (27): CountedHealthCheck, HealthCheckMode, HttpEndpointChecks, HttpHealthOptions, ProbeState, ProbeStreaks, Arc, Backend (+19 more)

### Community 1 - "Tes kesiapan HTTP"
Cohesion: 0.14
Nodes (25): accepts_ready_response_within_extended_timeout(), applies_consecutive_thresholds_to_ready_probes(), denies_status_to_a_blocked_client(), excludes_http_500_and_rejoins_after_http_200(), excludes_timed_out_ready_endpoint_and_rejoins_after_recovery(), fetch(), get(), HealthReply (+17 more)

### Community 2 - "Panduan operasi dan arsitektur"
Cohesion: 0.10
Nodes (24): Panduan agen, Pembaruan graf bersama melalui Git, graphify, Arsitektur workspace, Menempatkan perubahan, Mengembangkan struktur, Memeriksa perubahan, Menempatkan tes (+16 more)

### Community 3 - "Rotasi backend sehat"
Cohesion: 0.13
Nodes (19): BackendStatus, RoundRobinPool, Arc, Backend, Backends, Box, Duration, Mutex (+11 more)

### Community 4 - "Konfigurasi dan akses IP"
Cohesion: 0.11
Nodes (19): Args, main(), Box, Error, IpAddr, NonZeroU32, NonZeroUsize, Option (+11 more)

### Community 5 - "Penanganan request HTTP"
Cohesion: 0.17
Nodes (13): RequestBackend, ReverseProxy, Box, Error, Option, Result, Self, SocketAddr (+5 more)

### Community 6 - "Tes forwarding dan pemulihan"
Cohesion: 0.15
Nodes (16): alternates_backends_per_request_on_one_connection(), blocks_denied_and_unlisted_peers_before_forwarding(), fetch_marker(), forwards_post_and_returns_backend_response(), read_marker(), returns_bad_gateway_when_backend_is_offline(), routes_to_survivor_and_resumes_round_robin_after_recovery(), Server (+8 more)

### Community 7 - "Dependensi antar-crate"
Cohesion: 0.83
Nodes (4): proxy, proxy-balancer, proxy-http, proxy-security

## Knowledge Gaps
- **11 isolated node(s):** `Mencoba failover dan pemulihan`, `Memeriksa kesiapan aplikasi lewat HTTP`, `Melihat status tiap backend`, `Konfigurasi satu backend`, `Aturan akses IP` (+6 more)
  These have ≤1 connection - possible missing edges or undocumented components.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `RoundRobinPool` connect `Rotasi backend sehat` to `Probe dan penghitung kesehatan`, `Panduan operasi dan arsitektur`, `Penanganan request HTTP`?**
  _High betweenness centrality (0.286) - this node is a cross-community bridge._
- **Why does `ReverseProxy` connect `Penanganan request HTTP` to `Panduan operasi dan arsitektur`, `Rotasi backend sehat`, `Konfigurasi dan akses IP`?**
  _High betweenness centrality (0.181) - this node is a cross-community bridge._
- **Why does `Pengujian kesiapan melalui listener` connect `Tes kesiapan HTTP` to `Panduan operasi dan arsitektur`?**
  _High betweenness centrality (0.171) - this node is a cross-community bridge._
- **Are the 12 inferred relationships involving `start_proxy()` (e.g. with `alternates_backends_per_request_on_one_connection()` and `blocks_denied_and_unlisted_peers_before_forwarding()`) actually correct?**
  _`start_proxy()` has 12 INFERRED edges - model-reasoned connections that need verification._
- **What connects `Mencoba failover dan pemulihan`, `Memeriksa kesiapan aplikasi lewat HTTP`, `Melihat status tiap backend` to the rest of the system?**
  _11 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Probe dan penghitung kesehatan` be split into smaller, more focused modules?**
  _Cohesion score 0.09871794871794871 - nodes in this community are weakly interconnected._
- **Should `Tes kesiapan HTTP` be split into smaller, more focused modules?**
  _Cohesion score 0.14022988505747128 - nodes in this community are weakly interconnected._
## Cakupan ekstraksi

- Versi kode sumber yang dipindai: `8ab8d284`.
- Dari relasi mentah, 47 mengacu ke simpul yang tidak ditemukan. Graf tak berarah menyatukan 37 relasi tambahan yang berbagi pasangan simpul.
- Manifest workspace di root (`Cargo.toml`) tidak menghasilkan simpul. Keempat manifest package tercakup.
