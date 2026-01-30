//! CLI tool to create an API key in DynamoDB
//!
//! Usage:
//!   cargo run --bin create_api_key -- --user-id dev-user --name "Development Key"

use anyhow::Result;
use aws_sdk_dynamodb::types::AttributeValue;
use chrono::Utc;
use clap::Parser;
use uuid::Uuid;

/// Create a new API key in DynamoDB
#[derive(Parser, Debug)]
#[command(name = "create_api_key")]
#[command(about = "Create a new API key in DynamoDB")]
struct Args {
    /// User ID for the API key
    #[arg(short, long)]
    user_id: String,

    /// Human-readable name for the key
    #[arg(short, long)]
    name: String,

    /// Rate limit (requests per window)
    #[arg(long, default_value = "100")]
    rate_limit: i32,

    /// Service tier (default, flex, priority, reserved)
    #[arg(long, default_value = "default")]
    service_tier: String,

    /// Monthly budget limit in USD (optional)
    #[arg(long)]
    monthly_budget: Option<f64>,

    /// DynamoDB table name
    #[arg(long, default_value = "anthropic-proxy-api-keys")]
    table_name: String,

    /// DynamoDB endpoint URL (for local development)
    #[arg(long)]
    endpoint_url: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    let args = Args::parse();

    // Generate API key
    let api_key = format!("sk-{}", Uuid::new_v4());
    let created_at = Utc::now().timestamp();

    // Configure AWS SDK
    let mut config_builder = aws_config::from_env();

    // Check for endpoint URL from args or environment
    let endpoint_url = args.endpoint_url
        .or_else(|| std::env::var("DYNAMODB_ENDPOINT_URL").ok());

    if let Some(ref url) = endpoint_url {
        config_builder = config_builder.endpoint_url(url);
    }

    let aws_config = config_builder.load().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&aws_config);

    // Build the item
    let mut item = std::collections::HashMap::new();
    item.insert("api_key".to_string(), AttributeValue::S(api_key.clone()));
    item.insert("user_id".to_string(), AttributeValue::S(args.user_id.clone()));
    item.insert("name".to_string(), AttributeValue::S(args.name.clone()));
    item.insert("created_at".to_string(), AttributeValue::N(created_at.to_string()));
    item.insert("is_active".to_string(), AttributeValue::Bool(true));
    item.insert("rate_limit".to_string(), AttributeValue::N(args.rate_limit.to_string()));
    item.insert("service_tier".to_string(), AttributeValue::S(args.service_tier.clone()));
    item.insert("budget_used".to_string(), AttributeValue::N("0".to_string()));
    item.insert("budget_used_mtd".to_string(), AttributeValue::N("0".to_string()));

    if let Some(budget) = args.monthly_budget {
        item.insert("monthly_budget".to_string(), AttributeValue::N(budget.to_string()));
    }

    // Put item into DynamoDB
    dynamodb_client
        .put_item()
        .table_name(&args.table_name)
        .set_item(Some(item))
        .send()
        .await?;

    println!("\n✅ API Key created successfully!\n");
    println!("API Key: {}", api_key);
    println!("User ID: {}", args.user_id);
    println!("Name: {}", args.name);
    println!("Rate Limit: {} requests/minute", args.rate_limit);
    println!("Service Tier: {}", args.service_tier);
    if let Some(budget) = args.monthly_budget {
        println!("Monthly Budget: ${:.2}", budget);
    }
    println!("\nUse this key with:");
    println!("  ANTHROPIC_API_KEY=\"{}\"", api_key);

    Ok(())
}
