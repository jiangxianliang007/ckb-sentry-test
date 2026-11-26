use clap::{Parser, ValueEnum};
use sentry::{types::Dsn, ClientOptions, Level};
use std::{panic::catch_unwind, process::ExitCode, sync::Arc, time::Duration};

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
    #[arg(long, default_value = "ckb-sentry-test@0.1.0")]
    release: String,
    #[arg(long, default_value_t = 30)]
    timeout: u64,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let dsn = match args.dsn.parse::<Dsn>() {
        Ok(dsn) => dsn,
        Err(_) => {
            eprintln!("Invalid Sentry DSN");
            return ExitCode::FAILURE;
        }
    };
    let timeout = Duration::from_secs(args.timeout);
    let guard = sentry::init(ClientOptions {
        dsn: Some(dsn),
        release: Some(args.release.into()),
        attach_stacktrace: true,
        shutdown_timeout: timeout,
        before_send: Some(Arc::new(|mut event| {
            if let Some(name) = std::thread::current().name() {
                event.extra.insert("thread.name".into(), name.into());
            }
            println!("Event ID: {} (queued, not server acknowledgement)", event.event_id);
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
        let _ = catch_unwind(|| panic!("CKB Sentry compatibility test: intentional panic"));
    }
    if !guard.flush(Some(timeout)) {
        eprintln!("SDK flush timed out; check connectivity and Sentry logs");
        return ExitCode::FAILURE;
    }
    println!("SDK queue flushed. Verify the event IDs in Sentry Issues; flush does not prove ingestion.");
    ExitCode::SUCCESS
}
