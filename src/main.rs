use clap::{Parser, ValueEnum};
use sentry::{types::Dsn, ClientOptions, Level};
use std::{cell::Cell, io::IsTerminal, panic, process::ExitCode, sync::Arc, time::{Duration, Instant}};

const TEST_PANIC: &str = "CKB Sentry compatibility test: intentional panic";
thread_local! { static EXPECTED_PANIC: Cell<bool> = const { Cell::new(false) }; }

fn status(ok: bool) -> &'static str {
    match (ok, std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()) {
        (true, true) => "\x1b[32m[PASS]\x1b[0m",
        (false, true) => "\x1b[31m[FAIL]\x1b[0m",
        (true, false) => "[PASS]",
        (false, false) => "[FAIL]",
    }
}

fn is_expected(payload: &(dyn std::any::Any + Send)) -> bool {
    EXPECTED_PANIC.with(|flag| flag.get())
        && (payload.downcast_ref::<&str>().copied() == Some(TEST_PANIC)
            || payload.downcast_ref::<String>().map(String::as_str) == Some(TEST_PANIC))
}

fn install_console_hook() {
    // Install BEFORE sentry::init: SDK wraps this hook and captures the real panic first.
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if !is_expected(info.payload()) {
            previous(info);
        }
    }));
}


#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    Error,
    Panic,
    All,
}

#[derive(Parser)]
#[command(version, about = "Test Sentry ingestion using the CKB Rust SDK")]
struct Args {
    #[arg(long, env = "SENTRY_DSN", hide_env_values = true)]
    dsn: String,
    #[arg(long, value_enum, default_value = "error")]
    mode: Mode,
    #[arg(long, env = "SENTRY_ORG_IDENT")]
    org_ident: Option<String>,
    #[arg(long, default_value = concat!("ckb-sentry-test@", env!("CARGO_PKG_VERSION")))]
    release: String,
    #[arg(long, default_value_t = 30)]
    timeout: u64,
    #[arg(long, help = "Enable SDK diagnostics on stderr")]
    debug: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let total = Instant::now();
    let dsn = match args.dsn.parse::<Dsn>() {
        Ok(dsn) => dsn,
        Err(_) => {
            eprintln!("Invalid Sentry DSN");
            return ExitCode::FAILURE;
        }
    };
    let timeout = Duration::from_secs(args.timeout);
    println!("CKB Sentry Compatibility Test v{}", env!("CARGO_PKG_VERSION"));
    println!("SDK: sentry-rust 0.34.0");
    println!("Mode: {}", match args.mode { Mode::Error => "error", Mode::Panic => "panic", Mode::All => "all" });
    println!("Organization: {}", args.org_ident.as_deref().unwrap_or("(not set)"));
    if !args.debug { install_console_hook(); }
    let guard = sentry::init(ClientOptions {
        dsn: Some(dsn),
        release: Some(args.release.into()),
        attach_stacktrace: true,
        shutdown_timeout: timeout,
        debug: args.debug,
        before_send: Some(Arc::new(|mut event| {
            if let Some(name) = std::thread::current().name() {
                event.extra.insert("thread.name".into(), name.into());
            }
            println!("{} {} event captured locally", status(true), if event.level == Level::Fatal { "Intentional panic" } else { "Error" });
            println!("Event ID: {} (not server acknowledgement)", event.event_id);
            Some(event)
        })),
        ..Default::default()
    });
    if !guard.is_enabled() {
        eprintln!("Sentry client is disabled");
        return ExitCode::FAILURE;
    }
    sentry::configure_scope(|scope| {
        scope.set_tag("release.pre", false);
        scope.set_tag("release.dirty", false);
        scope.set_tag("compatibility_test", "true");
        if let Some(org) = &args.org_ident {
            scope.set_tag("org_ident", org);
        }
    });
    if matches!(args.mode, Mode::Error | Mode::All) {
        sentry::capture_message("CKB Sentry compatibility test: error", Level::Error);
    }
    if matches!(args.mode, Mode::Panic | Mode::All) {
        EXPECTED_PANIC.with(|flag| flag.set(true));
        let result = panic::catch_unwind(|| panic!("{}", TEST_PANIC));
        if let Err(payload) = result {
            let expected = is_expected(payload.as_ref());
            EXPECTED_PANIC.with(|flag| flag.set(false));
            if !expected { panic::resume_unwind(payload); }
        } else {
            EXPECTED_PANIC.with(|flag| flag.set(false));
            eprintln!("{} Expected panic did not occur", status(false));
            return ExitCode::FAILURE;
        }
    }
    // SDK 0.34.0 reqwest transport returns recv_timeout(...).is_err().
    // Keep the SDK pinned for CKB compatibility; true means wait failed here.
    let started = std::time::Instant::now();
    if guard.flush(Some(timeout)) {
        eprintln!("{} SDK flush wait failed; check connectivity and retry with --debug", status(false));
        return ExitCode::FAILURE;
    }
    println!("{} SDK queue flushed in {:?}", status(true), started.elapsed());
    println!("Test completed in {:?}. Verify both event types as requested in Sentry; flush does not prove ingestion.", total.elapsed());
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_marked_test_panic_is_expected() {
        assert!(!is_expected(&TEST_PANIC));
        EXPECTED_PANIC.with(|flag| flag.set(true));
        assert!(is_expected(&TEST_PANIC));
        assert!(is_expected(&TEST_PANIC.to_string()));
        assert!(!is_expected(&"unexpected panic"));
        EXPECTED_PANIC.with(|flag| flag.set(false));
    }
}
