//! LLM API Converter
//!
//! A high-performance API gateway that unifies different AI provider APIs
//! (Anthropic, OpenAI, Bedrock, etc.)

use anyhow::Result;
use llm_api_converter::{
    config::{Environment, Settings},
    logging::SizeBasedRollingWriter,
    server::App,
};
use clap::Parser;
use std::path::PathBuf;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, Layer};

/// LLM API Converter
///
/// A high-performance API gateway that unifies different AI provider APIs.
#[derive(Parser, Debug)]
#[command(name = "llm-api-converter")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to listen on (overrides PORT env var)
    #[arg(short, long)]
    port: Option<u16>,

    /// Host to bind to (overrides HOST env var)
    #[arg(long)]
    host: Option<String>,

    /// Log level: trace, debug, info, warn, error (overrides LOG_LEVEL env var)
    #[arg(long)]
    log_level: Option<String>,

    /// Environment: dev, staging, prod (overrides ENVIRONMENT env var)
    #[arg(short, long)]
    env: Option<Environment>,

    /// Print all request prompts to stdout (for debugging)
    #[arg(long)]
    print_prompts: bool,

    /// Log file path for JSON logs (enables file logging with 10MB rotation)
    /// Example: --log-file /var/log/proxy/app.log
    #[arg(long)]
    log_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration first (before logging, so we can use log_level)
    let mut settings = Settings::load()?;

    // Override settings with CLI arguments
    if let Some(port) = args.port {
        settings.port = port;
    }
    if let Some(host) = args.host {
        settings.host = host;
    }
    if let Some(log_level) = args.log_level {
        settings.log_level = log_level;
    }
    if let Some(env) = args.env {
        settings.environment = env;
    }
    if args.print_prompts {
        settings.print_prompts = true;
    }

    // Generate ephemeral API key for development
    let ephemeral_key = settings.generate_ephemeral_key();

    // Initialize tracing subscriber with JSON output
    init_tracing(&settings.log_level, args.log_file.as_ref());

    // Print ephemeral API key to console
    println!("\n{}", "=".repeat(60));
    println!("  Ephemeral API Key (valid for this session only):");
    println!("  {}", ephemeral_key);
    println!("{}\n", "=".repeat(60));
    println!("  Usage:");
    println!("    export ANTHROPIC_API_KEY=\"{}\"", ephemeral_key);
    println!("    export ANTHROPIC_BASE_URL=\"http://{}:{}\"\n", settings.host, settings.port);
    println!("{}\n", "=".repeat(60));

    tracing::info!(
        app_name = %settings.app_name,
        version = %settings.app_version,
        environment = %settings.environment,
        host = %settings.host,
        port = %settings.port,
        "Starting application"
    );

    // Build the application
    let app = App::new(settings).await?;

    // Run the server with graceful shutdown
    app.run_with_graceful_shutdown().await?;

    tracing::info!("Application shutdown complete");

    Ok(())
}

/// Initialize tracing subscriber with the specified log level
/// Optionally writes to a rolling log file (10MB per file, max 10 files)
fn init_tracing(log_level: &str, log_file: Option<&PathBuf>) {
    // Build filter from RUST_LOG env var or use provided log level
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    // Console layer - always enabled, JSON format
    let console_layer = fmt::layer()
        .json()
        .with_filter(filter.clone());

    // Build the subscriber
    let subscriber = tracing_subscriber::registry().with(console_layer);

    // Add file layer if log_file is specified
    if let Some(path) = log_file {
        // Create size-based rolling file writer (10MB per file, max 10 files)
        let file_writer = SizeBasedRollingWriter::with_defaults(path)
            .expect("Failed to create log file writer");

        // File layer - JSON format, writes to rolling file
        let file_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

        let file_layer = fmt::layer()
            .json()
            .with_writer(file_writer)
            .with_filter(file_filter);

        subscriber.with(file_layer).init();

        eprintln!("Logging to file: {} (10MB rotation, max 10 files)", path.display());
    } else {
        subscriber.init();
    }
}
