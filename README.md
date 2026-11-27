# ckb-sentry-test

Test self-hosted Sentry ingestion with Rust SDK 0.34.0, matching the SDK declared by [CKB](https://github.com/nervosnetwork/ckb/blob/v0.210.0/Cargo.toml). Uses the SDK's normal transport and panic integration, with CKB-style organization and release tags. This is a standalone synthetic test, not the CKB node itself.

## Download

Download the Linux amd64 or arm64 archive from [Actions](https://github.com/jiangxianliang007/ckb-sentry-test/actions) (each main build) or [Releases](https://github.com/jiangxianliang007/ckb-sentry-test/releases) (version tags). Extract the artifact ZIP if downloading from Actions, verify the SHA256 file with sha256sum -c, and extract the tar.gz archive. Built on Ubuntu 22.04; intended for Ubuntu 22.04 or newer. No Rust toolchain needed on the target host.

## Usage

```sh
export SENTRY_DSN='http://PUBLIC_KEY@sentry.example.org/4'
./ckb-sentry-test --mode all --org-ident ckb-dev
```

Use --mode error or --mode panic for individual tests. Panic is intentional and caught after the SDK panic hook runs; the test continues to flush. An event ID or successful flush does not guarantee server acceptance: check the events in Sentry. The compatibility_test tag identifies synthetic events. Tests can trigger project alerts.

DSN can also be passed with --dsn, but environment variables avoid putting it in command-line arguments. No DSN is embedded in the binary or CI.

## Build and release

Run cargo build --release locally if desired. GitHub Actions builds both architectures on pushes to main and on version tags. To publish binaries to Releases, push a tag such as v0.1.0. The workflow also supports manual builds.

## SDK 0.34.0 flush behavior

The default reqwest transport in SDK 0.34.0 returns `recv_timeout(...).is_err()` from flush, the inverse of the documented contract. Version 0.1.1 accounts for that behavior without changing the SDK version. Use `--debug` to inspect SDK diagnostics; output can contain event data, so do not share it unredacted. A drained queue still does not guarantee HTTP acceptance or ingestion.

## Console output (v0.1.2)

The intentional test panic still runs through the SDK panic hook with its original stack trace. Only its default console message is suppressed; unrelated panics retain normal output. Use `--debug` to retain the original panic diagnostics. PASS indicates local capture or queue processing, not server acceptance. Colors are used only on interactive stdout and can be disabled with NO_COLOR.
