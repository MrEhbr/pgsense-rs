# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

---
## [0.6.1](https://github.com/MrEhbr/pgsense-rs/compare/v0.6.0..v0.6.1) - 2026-05-06

### CI/CD

- skip checks on docs, charts, and sibling workflows - ([2c851f9](https://github.com/MrEhbr/pgsense-rs/commit/2c851f983eb670877c7d1ed039e85018e653673e)) `+16 / -0 across 1 file(s)` - Aleksei Burmistrov
- skip checks on release-bump commit, trigger publish on tag create - ([7ef7aa6](https://github.com/MrEhbr/pgsense-rs/commit/7ef7aa60358175858263c2cfe6f90aff66501a23)) `+5 / -4 across 2 file(s)` - Aleksei Burmistrov

### Documentation

- document etl schema bootstrap permissions - ([c674234](https://github.com/MrEhbr/pgsense-rs/commit/c67423404585632e74fb9fa69ef17699a6dbf43d)) `+161 / -11 across 2 file(s)` - Aleksei Burmistrov

### Statistics

- 3 commit(s) contributed to the release.
- 0 day(s) between first and last commit.
- 3 commit(s) parsed as conventional.
- Diff totals: +182 / -15 across 5 file change(s) (sum across commits, may double-count files touched in multiple commits).
- 1 day(s) since the previous release.

## [0.6.0](https://github.com/MrEhbr/pgsense-rs/compare/v0.5.0..v0.6.0) - 2026-05-05

### Bug Fixes

- **(rand)** migrate to RngExt trait for rand 0.10 - ([d539c88](https://github.com/MrEhbr/pgsense-rs/commit/d539c887a92432ef5ee9c0e3342bfd90a13526f0)) `+1 / -1 across 1 file(s)` - Aleksei Burmistrov

### CI/CD

- add cargo-deny audit workflow - ([85e4fe6](https://github.com/MrEhbr/pgsense-rs/commit/85e4fe6868352aa8b89e26a928e83fb4fda06890)) `+40 / -0 across 1 file(s)` - Aleksei Burmistrov

### Documentation

- **(config)** update inline pipeline and rule validator comments - ([949bba6](https://github.com/MrEhbr/pgsense-rs/commit/949bba68f3ab4c11483bd19c530f383d987c941d)) `+8 / -6 across 2 file(s)` - Aleksei Burmistrov
- add mdBook documentation site - ([a236d96](https://github.com/MrEhbr/pgsense-rs/commit/a236d96ee9a675e9fab5ba8e1699e9cf32e8054a)) `+3009 / -140 across 45 file(s)` - Aleksei Burmistrov
- add mdBook documentation site - ([a9ce885](https://github.com/MrEhbr/pgsense-rs/commit/a9ce885dfa5f0eae9798101d660c926c4df686e6)) `+1 / -1 across 1 file(s)` - Aleksei Burmistrov
- add CONTRIBUTING, SECURITY, and GitHub templates - ([b8a631f](https://github.com/MrEhbr/pgsense-rs/commit/b8a631fb7eae6ca40dc30715fbbcbaf627052dc8)) `+204 / -0 across 7 file(s)` - Aleksei Burmistrov
- switch config examples and user docs to Secret type shape - ([573404e](https://github.com/MrEhbr/pgsense-rs/commit/573404ed50cf240bab1c4cfe118a7d1527769f64)) `+67 / -43 across 13 file(s)` - Aleksei Burmistrov

### Features

- **(rules)** add phone/email/IBAN validators for regex rules - ([80deb89](https://github.com/MrEhbr/pgsense-rs/commit/80deb89ef8efdd3225a73f479ae040bfd576443d)) `+21 / -0 across 2 file(s)` - Aleksei Burmistrov

### Miscellaneous Chores

- **(changelog)** add per-commit and total diff stats to git-cliff template - ([5bbeee0](https://github.com/MrEhbr/pgsense-rs/commit/5bbeee0e43683a9fa19082e020bbc19a1e4a4038)) `+22 / -8 across 1 file(s)` - Aleksei Burmistrov
- **(deny)** allow CC0-1.0, CDLA-Permissive-2.0, MPL-2.0 licenses - ([3f028eb](https://github.com/MrEhbr/pgsense-rs/commit/3f028eb4e529eb0e810d58caabeeb824e3f6610d)) `+3 / -0 across 1 file(s)` - Aleksei Burmistrov
- **(deps)** bump the actions group across 1 directory with 8 updates - ([692f89e](https://github.com/MrEhbr/pgsense-rs/commit/692f89ee2395804f827398164421be742b70b63a)) `+16 / -16 across 5 file(s)` - dependabot[bot]
- **(deps)** bump tokio in the patch-and-minor group across 1 directory - ([2002521](https://github.com/MrEhbr/pgsense-rs/commit/2002521b44e3574868312f7ac97062250c00a161)) `+2 / -2 across 1 file(s)` - dependabot[bot]
- **(deps)** bump rand from 0.9.4 to 0.10.1 - ([dff2d41](https://github.com/MrEhbr/pgsense-rs/commit/dff2d4164b886206784668933e41787f4e4d56c5)) `+2 / -2 across 2 file(s)` - dependabot[bot]
- **(fuzz)** refresh lockfile to clear vuln alerts - ([265de3d](https://github.com/MrEhbr/pgsense-rs/commit/265de3d9b7e48ca565a80f5341b5daacef8b7dda)) `+829 / -394 across 1 file(s)` - Aleksei Burmistrov
- **(nix)** bump flake inputs - ([55fa508](https://github.com/MrEhbr/pgsense-rs/commit/55fa5086a8d978441d78c01b1124e914fa7cd370)) `+14 / -14 across 1 file(s)` - Aleksei Burmistrov
- add package metadata and pin Dockerfile base image - ([f82985a](https://github.com/MrEhbr/pgsense-rs/commit/f82985aa094c4a10b991c2ed20811dde04cefab5)) `+21 / -7 across 3 file(s)` - Aleksei Burmistrov
- gitignore CLAUDE.md before going public - ([58f602b](https://github.com/MrEhbr/pgsense-rs/commit/58f602b844161df7fdac5f6144f2ade009c5f7cd)) `+1 / -80 across 2 file(s)` - Aleksei Burmistrov
- add dependabot config for cargo, github-actions, docker - ([655e73f](https://github.com/MrEhbr/pgsense-rs/commit/655e73f014aa49b496096d21b89f6e89e3c4a288)) `+31 / -0 across 1 file(s)` - Aleksei Burmistrov

### Refactoring

- **(config)** unify secret fields under Secret type - ([c0868fa](https://github.com/MrEhbr/pgsense-rs/commit/c0868fa975524084d76b09c9dd37e53ed3567023)) `+600 / -355 across 12 file(s)` - Aleksei Burmistrov

### Statistics

- 19 commit(s) contributed to the release.
- 1 day(s) between first and last commit.
- 19 commit(s) parsed as conventional.
- Diff totals: +4892 / -1069 across 102 file change(s) (sum across commits, may double-count files touched in multiple commits).
- 1 day(s) since the previous release.

## [0.5.0](https://github.com/MrEhbr/pgsense-rs/compare/v0.4.0..v0.5.0) - 2026-05-04

### Features

- **(cli)** add validate command for config and connectivity checks - ([d37414a](https://github.com/MrEhbr/pgsense-rs/commit/d37414aaea858de4f5f03855b17656567a10ee05)) `+1149 / -94 across 13 file(s)` - Aleksei Burmistrov
- **(profiling)** add per-rule and per-phase scan duration metrics - ([bc5a15e](https://github.com/MrEhbr/pgsense-rs/commit/bc5a15e2349f97ea6dc4c23646be108f1dbc6bc0)) `+292 / -54 across 15 file(s)` - Aleksei Burmistrov
- **(rules)** add email address detector - ([4a357c4](https://github.com/MrEhbr/pgsense-rs/commit/4a357c46994af0db949fce9febbaab08787007d8)) `+598 / -160 across 35 file(s)` - Aleksei Burmistrov
- **(rules)** add IBAN builtin detector - ([db07246](https://github.com/MrEhbr/pgsense-rs/commit/db07246d3a1d87231aa39547900d0a1183f8b69c)) `+329 / -3 across 25 file(s)` - Aleksei Burmistrov
- **(rules)** add bench subcommand - ([5fd6793](https://github.com/MrEhbr/pgsense-rs/commit/5fd6793cf9d60a7e8ae67409256ae17da2c84021)) `+377 / -6 across 4 file(s)` - Aleksei Burmistrov
- **(telemetry)** add optional OpenTelemetry tracing via otel feature flag - ([6bcd41e](https://github.com/MrEhbr/pgsense-rs/commit/6bcd41e2f01abdd7ac48655d2068470523c52c5b)) `+547 / -85 across 16 file(s)` - Aleksei Burmistrov
- support glob patterns in scan filter and rule scope config - ([38d7f47](https://github.com/MrEhbr/pgsense-rs/commit/38d7f4787627d602540f5cd022d0d58abcb2687f)) `+317 / -78 across 12 file(s)` - Aleksei Burmistrov

### Miscellaneous Chores

- **(changelog)** add release statistics block - ([387d1a9](https://github.com/MrEhbr/pgsense-rs/commit/387d1a9d78eb69e9ce22a9c44565bc2e15f34b6f)) `+10 / -1 across 1 file(s)` - Aleksei Burmistrov

### Refactoring

- **(pipeline)** bump etl, use upstream stores, scan partial UPDATE rows - ([814d84b](https://github.com/MrEhbr/pgsense-rs/commit/814d84be462c975d8f2f3197d51da518ca4d7113)) `+1697 / -3054 across 28 file(s)` - Aleksei Burmistrov

### Statistics

- 9 commit(s) contributed to the release.
- 42 day(s) between first and last commit.
- 9 commit(s) parsed as conventional.
- Diff totals: +5316 / -3535 across 149 file change(s) (sum across commits, may double-count files touched in multiple commits).
- 46 day(s) since the previous release.

## [0.4.0](https://github.com/MrEhbr/pgsense-rs/compare/v0.3.0..v0.4.0) - 2026-03-19

### Features

- **(helm)** expose full config and fix image repository - ([c4b60fc](https://github.com/MrEhbr/pgsense-rs/commit/c4b60fc46c97dda16910575112315b48c01b65f6)) `+117 / -3 across 3 file(s)` - Aleksei Burmistrov

### Statistics

- 1 commit(s) contributed to the release.
- 0 day(s) between first and last commit.
- 1 commit(s) parsed as conventional.
- Diff totals: +117 / -3 across 3 file change(s) (sum across commits, may double-count files touched in multiple commits).

## [0.3.0](https://github.com/MrEhbr/pgsense-rs/compare/v0.2.0..v0.3.0) - 2026-03-19

### CI/CD

- **(helm)** publish chart to GHCR OCI registry on release - ([cf154f4](https://github.com/MrEhbr/pgsense-rs/commit/cf154f44e0fb1f4eddbdec09cc5adb96f46fed98)) `+24 / -2 across 3 file(s)` - Aleksei Burmistrov

### Features

- **(helm)** use adaptive intervals and add process panels - ([6b31652](https://github.com/MrEhbr/pgsense-rs/commit/6b3165281769b2cbd8487e1ec8aef1eab1bbd842)) `+198 / -16 across 1 file(s)` - Aleksei Burmistrov

### Statistics

- 2 commit(s) contributed to the release.
- 0 day(s) between first and last commit.
- 2 commit(s) parsed as conventional.
- Diff totals: +222 / -18 across 4 file change(s) (sum across commits, may double-count files touched in multiple commits).
- 1 day(s) since the previous release.

## [0.2.0](https://github.com/MrEhbr/pgsense-rs/compare/v0.1.0..v0.2.0) - 2026-03-18

### Bug Fixes

- **(ci)** disable macOS aarch64-apple-darwin cross-compilation build - ([150be2f](https://github.com/MrEhbr/pgsense-rs/commit/150be2f186c79994af9d0d350918223ba00b79b6)) `+1 / -0 across 1 file(s)` - Aleksei Burmistrov

### Features

- **(config)** add password_file support and PGSENSE prefix - ([aef34a9](https://github.com/MrEhbr/pgsense-rs/commit/aef34a902b2fd32313fde462bc2b3c338ea90cbd)) `+152 / -66 across 8 file(s)` - Aleksei Burmistrov
- **(helm)** add Helm chart - ([d364acb](https://github.com/MrEhbr/pgsense-rs/commit/d364acbcf01b91a518587f0f348c02c7aa73deee)) `+472 / -0 across 9 file(s)` - Aleksei Burmistrov
- **(helm)** add Grafana dashboard - ([5c5d633](https://github.com/MrEhbr/pgsense-rs/commit/5c5d633e03fa94988641f665ef883762e3d86100)) `+1065 / -16 across 5 file(s)` - Aleksei Burmistrov

### Miscellaneous Chores

- **(release)** bundle rules.toml in Docker image and archives - ([ca736a5](https://github.com/MrEhbr/pgsense-rs/commit/ca736a5f49f9d55b186e9b3900a103955be82bd6)) `+8 / -2 across 2 file(s)` - Aleksei Burmistrov

### Statistics

- 5 commit(s) contributed to the release.
- 1 day(s) between first and last commit.
- 5 commit(s) parsed as conventional.
- Diff totals: +1698 / -84 across 25 file change(s) (sum across commits, may double-count files touched in multiple commits).
- 1 day(s) since the previous release.

## [0.1.0] - 2026-03-17

### Bug Fixes

- **(tests)** retry insert in flaky multi-db pipeline test - ([5346704](https://github.com/MrEhbr/pgsense-rs/commit/53467046af651178a4306f2229855e9aa00733ab)) `+23 / -9 across 2 file(s)` - Aleksei Burmistrov

### Features

- **(alerts)** add PostgreSQL alert channel - ([84ffacb](https://github.com/MrEhbr/pgsense-rs/commit/84ffacb0c6f991cca9975188921901057785b5db)) `+408 / -18 across 9 file(s)` - Aleksei Burmistrov
- **(alerts)** add name/channels fields for per-rule channel routing - ([17510b6](https://github.com/MrEhbr/pgsense-rs/commit/17510b6bed9f940db37cd2ec1945a9eb9693c71a)) `+30 / -0 across 13 file(s)` - Aleksei Burmistrov
- **(alerts)** implement per-rule alert channel routing - ([66a7611](https://github.com/MrEhbr/pgsense-rs/commit/66a761159a8f8b5f5f6e79dd54cc598924a536d1)) `+192 / -40 across 5 file(s)` - Aleksei Burmistrov
- **(bench)** add docker load test stack with prometheus and grafana - ([1056f0a](https://github.com/MrEhbr/pgsense-rs/commit/1056f0a9262752d3af7440853d6954c973fedcb1)) `+632 / -53 across 19 file(s)` - Aleksei Burmistrov
- **(metrics)** add batch size, queue depth, and dispatch duration metrics - ([d81b31d](https://github.com/MrEhbr/pgsense-rs/commit/d81b31d0639c5496b401e1b6202c4e13ed62ce26)) `+49 / -6 across 4 file(s)` - Aleksei Burmistrov
- **(pipeline)** add multi-database support with supervisor - ([97eb336](https://github.com/MrEhbr/pgsense-rs/commit/97eb3360ee92ee1a36c2e4dcc8cc73eb4ec8536a)) `+1560 / -693 across 32 file(s)` - Aleksei Burmistrov
- **(rules)** add per-rule scope filtering - ([9495b16](https://github.com/MrEhbr/pgsense-rs/commit/9495b16cfe3091f3cea1b1f6363215364e22774a)) `+196 / -4 across 7 file(s)` - Aleksei Burmistrov
- **(rules)** add secrets detection for popular cloud and SaaS services - ([be593db](https://github.com/MrEhbr/pgsense-rs/commit/be593db6d17c9b7444fac1bcb2ddfd229ac9a621)) `+166 / -26 across 1 file(s)` - Aleksei Burmistrov
- initial commit - ([6373a32](https://github.com/MrEhbr/pgsense-rs/commit/6373a323617f65b6cfe30e1fce36353247f8f30e)) `+16934 / -0 across 95 file(s)` - Aleksei Burmistrov
- add Slack alert channel with batched delivery - ([a355de5](https://github.com/MrEhbr/pgsense-rs/commit/a355de536b30f184b4e8d6d950bd9ac126b4b488)) `+624 / -66 across 9 file(s)` - Aleksei Burmistrov
- add phone number detector and split detectors module - ([1953446](https://github.com/MrEhbr/pgsense-rs/commit/1953446afe855f077dc38452adabbe895d2d1d31)) `+944 / -227 across 33 file(s)` - Aleksei Burmistrov

### Miscellaneous Chores

- **(bench)** update grafana dashboard panels - ([3e54338](https://github.com/MrEhbr/pgsense-rs/commit/3e54338d744d788bd8063a470fcb8bba7f1243f2)) `+217 / -9 across 1 file(s)` - Aleksei Burmistrov
- replace lazy_static with std::sync::LazyLock - ([577e94a](https://github.com/MrEhbr/pgsense-rs/commit/577e94a91abe2ba1c967904aaa2a3289d3397a21)) `+141 / -59 across 5 file(s)` - Aleksei Burmistrov
- add typos to pre-commit hook and fix HashiCorp allow-list - ([8ea1761](https://github.com/MrEhbr/pgsense-rs/commit/8ea1761273fc345f14c119d9f4fdf68ee28e86ab)) `+6 / -0 across 1 file(s)` - Aleksei Burmistrov
- reorganize config files and update docs - ([086eb6b](https://github.com/MrEhbr/pgsense-rs/commit/086eb6b60452e3f78f200cc0f21cee55df34e0ec)) `+192 / -242 across 8 file(s)` - Aleksei Burmistrov

### Performance

- **(rules)** eliminate allocations in hot scan path - ([fa76046](https://github.com/MrEhbr/pgsense-rs/commit/fa76046a0676963b2cabde8bd3cd1fba48dea65f)) `+146 / -121 across 8 file(s)` - Aleksei Burmistrov
- **(rules)** replace phonenumber with rlibphonenumber - ([9b491b8](https://github.com/MrEhbr/pgsense-rs/commit/9b491b8584ed1d74019806be744f2afea84d3b7d)) `+176 / -219 across 5 file(s)` - Aleksei Burmistrov

### Refactoring

- **(alerts)** reduce allocations and simplify - ([8377ed0](https://github.com/MrEhbr/pgsense-rs/commit/8377ed0f089c01fa122db8cf13efbb553bc15167)) `+91 / -83 across 5 file(s)` - Aleksei Burmistrov
- **(metrics)** migrate to prometheus crate - ([1e5c428](https://github.com/MrEhbr/pgsense-rs/commit/1e5c42874efe020d34269f87579607cedf51ce8c)) `+340 / -209 across 9 file(s)` - Aleksei Burmistrov
- use SecretString for passwords and auth headers - ([d0b9e85](https://github.com/MrEhbr/pgsense-rs/commit/d0b9e858f75382c0a18f401b3ee8702325631463)) `+27 / -33 across 5 file(s)` - Aleksei Burmistrov
- clean up code comments - ([ab398c7](https://github.com/MrEhbr/pgsense-rs/commit/ab398c71fa82ae0ce03269f166e9c6c2e00e6630)) `+0 / -13 across 5 file(s)` - Aleksei Burmistrov

### Statistics

- 22 commit(s) contributed to the release.
- 33 day(s) between first and last commit.
- 22 commit(s) parsed as conventional.
- Diff totals: +23094 / -2130 across 281 file change(s) (sum across commits, may double-count files touched in multiple commits).


