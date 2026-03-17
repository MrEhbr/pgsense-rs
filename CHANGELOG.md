# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

---
## [0.1.0] - 2026-03-17

### Bug Fixes

- **(tests)** retry insert in flaky multi-db pipeline test - ([5346704](https://github.com/MrEhbr/pgsense-rs/commit/53467046af651178a4306f2229855e9aa00733ab)) - Aleksei Burmistrov
### Features

- **(alerts)** add PostgreSQL alert channel - ([84ffacb](https://github.com/MrEhbr/pgsense-rs/commit/84ffacb0c6f991cca9975188921901057785b5db)) - Aleksei Burmistrov
- **(alerts)** add name/channels fields for per-rule channel routing - ([17510b6](https://github.com/MrEhbr/pgsense-rs/commit/17510b6bed9f940db37cd2ec1945a9eb9693c71a)) - Aleksei Burmistrov
- **(alerts)** implement per-rule alert channel routing - ([66a7611](https://github.com/MrEhbr/pgsense-rs/commit/66a761159a8f8b5f5f6e79dd54cc598924a536d1)) - Aleksei Burmistrov
- **(bench)** add docker load test stack with prometheus and grafana - ([1056f0a](https://github.com/MrEhbr/pgsense-rs/commit/1056f0a9262752d3af7440853d6954c973fedcb1)) - Aleksei Burmistrov
- **(metrics)** add batch size, queue depth, and dispatch duration metrics - ([d81b31d](https://github.com/MrEhbr/pgsense-rs/commit/d81b31d0639c5496b401e1b6202c4e13ed62ce26)) - Aleksei Burmistrov
- **(pipeline)** add multi-database support with supervisor - ([97eb336](https://github.com/MrEhbr/pgsense-rs/commit/97eb3360ee92ee1a36c2e4dcc8cc73eb4ec8536a)) - Aleksei Burmistrov
- **(rules)** add per-rule scope filtering - ([9495b16](https://github.com/MrEhbr/pgsense-rs/commit/9495b16cfe3091f3cea1b1f6363215364e22774a)) - Aleksei Burmistrov
- **(rules)** add secrets detection for popular cloud and SaaS services - ([be593db](https://github.com/MrEhbr/pgsense-rs/commit/be593db6d17c9b7444fac1bcb2ddfd229ac9a621)) - Aleksei Burmistrov- initial commit - ([6373a32](https://github.com/MrEhbr/pgsense-rs/commit/6373a323617f65b6cfe30e1fce36353247f8f30e)) - Aleksei Burmistrov
- add Slack alert channel with batched delivery - ([a355de5](https://github.com/MrEhbr/pgsense-rs/commit/a355de536b30f184b4e8d6d950bd9ac126b4b488)) - Aleksei Burmistrov
- add phone number detector and split detectors module - ([1953446](https://github.com/MrEhbr/pgsense-rs/commit/1953446afe855f077dc38452adabbe895d2d1d31)) - Aleksei Burmistrov

### Miscellaneous Chores

- **(bench)** update grafana dashboard panels - ([3e54338](https://github.com/MrEhbr/pgsense-rs/commit/3e54338d744d788bd8063a470fcb8bba7f1243f2)) - Aleksei Burmistrov- replace lazy_static with std::sync::LazyLock - ([577e94a](https://github.com/MrEhbr/pgsense-rs/commit/577e94a91abe2ba1c967904aaa2a3289d3397a21)) - Aleksei Burmistrov
- add typos to pre-commit hook and fix HashiCorp allow-list - ([8ea1761](https://github.com/MrEhbr/pgsense-rs/commit/8ea1761273fc345f14c119d9f4fdf68ee28e86ab)) - Aleksei Burmistrov
- reorganize config files and update docs - ([086eb6b](https://github.com/MrEhbr/pgsense-rs/commit/086eb6b60452e3f78f200cc0f21cee55df34e0ec)) - Aleksei Burmistrov

### Performance

- **(rules)** eliminate allocations in hot scan path - ([fa76046](https://github.com/MrEhbr/pgsense-rs/commit/fa76046a0676963b2cabde8bd3cd1fba48dea65f)) - Aleksei Burmistrov
- **(rules)** replace phonenumber with rlibphonenumber - ([9b491b8](https://github.com/MrEhbr/pgsense-rs/commit/9b491b8584ed1d74019806be744f2afea84d3b7d)) - Aleksei Burmistrov
### Refactoring

- **(alerts)** reduce allocations and simplify - ([8377ed0](https://github.com/MrEhbr/pgsense-rs/commit/8377ed0f089c01fa122db8cf13efbb553bc15167)) - Aleksei Burmistrov
- **(metrics)** migrate to prometheus crate - ([1e5c428](https://github.com/MrEhbr/pgsense-rs/commit/1e5c42874efe020d34269f87579607cedf51ce8c)) - Aleksei Burmistrov- use SecretString for passwords and auth headers - ([d0b9e85](https://github.com/MrEhbr/pgsense-rs/commit/d0b9e858f75382c0a18f401b3ee8702325631463)) - Aleksei Burmistrov
- clean up code comments - ([ab398c7](https://github.com/MrEhbr/pgsense-rs/commit/ab398c71fa82ae0ce03269f166e9c6c2e00e6630)) - Aleksei Burmistrov


