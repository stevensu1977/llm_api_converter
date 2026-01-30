//! CLI tool to create DynamoDB tables for the anthropic-bedrock-proxy
//!
//! Usage:
//!   cargo run --bin setup_tables
//!
//! For local development with DynamoDB Local:
//!   DYNAMODB_ENDPOINT_URL=http://localhost:8001 cargo run --bin setup_tables

use anyhow::Result;
use aws_sdk_dynamodb::types::{
    AttributeDefinition, BillingMode, KeySchemaElement, KeyType, ScalarAttributeType,
};
use clap::Parser;

/// Create DynamoDB tables for the anthropic-bedrock-proxy
#[derive(Parser, Debug)]
#[command(name = "setup_tables")]
#[command(about = "Create DynamoDB tables for the anthropic-bedrock-proxy")]
struct Args {
    /// DynamoDB endpoint URL (for local development)
    #[arg(long)]
    endpoint_url: Option<String>,

    /// Table name prefix
    #[arg(long, default_value = "anthropic-proxy")]
    prefix: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists
    dotenvy::dotenv().ok();

    let args = Args::parse();

    // Configure AWS SDK
    let mut config_builder = aws_config::from_env();

    // Check for endpoint URL from args or environment
    let endpoint_url = args
        .endpoint_url
        .or_else(|| std::env::var("DYNAMODB_ENDPOINT_URL").ok());

    if let Some(ref url) = endpoint_url {
        config_builder = config_builder.endpoint_url(url);
        println!("Using DynamoDB endpoint: {}", url);
    }

    let aws_config = config_builder.load().await;
    let client = aws_sdk_dynamodb::Client::new(&aws_config);

    // Define table configurations
    let tables = [
        (
            format!("{}-api-keys", args.prefix),
            "api_key",
            ScalarAttributeType::S,
        ),
        (
            format!("{}-usage-stats", args.prefix),
            "api_key",
            ScalarAttributeType::S,
        ),
        (
            format!("{}-model-mapping", args.prefix),
            "anthropic_model_id",
            ScalarAttributeType::S,
        ),
        (
            format!("{}-model-pricing", args.prefix),
            "model_id",
            ScalarAttributeType::S,
        ),
    ];

    println!("\n🚀 Setting up DynamoDB tables...\n");

    for (table_name, pk_name, pk_type) in tables {
        match create_table(&client, &table_name, pk_name, pk_type).await {
            Ok(true) => println!("✅ Created table: {}", table_name),
            Ok(false) => println!("⏭️  Table already exists: {}", table_name),
            Err(e) => println!("❌ Failed to create table {}: {}", table_name, e),
        }
    }

    // Create usage table with partition key + sort key
    let usage_table = format!("{}-usage", args.prefix);
    match create_usage_table(&client, &usage_table).await {
        Ok(true) => println!("✅ Created table: {}", usage_table),
        Ok(false) => println!("⏭️  Table already exists: {}", usage_table),
        Err(e) => println!("❌ Failed to create table {}: {}", usage_table, e),
    }

    println!("\n✅ Table setup complete!\n");

    Ok(())
}

async fn create_table(
    client: &aws_sdk_dynamodb::Client,
    table_name: &str,
    pk_name: &str,
    pk_type: ScalarAttributeType,
) -> Result<bool> {
    // Check if table already exists
    let tables = client.list_tables().send().await?;
    if tables.table_names().contains(&table_name.to_string()) {
        return Ok(false);
    }

    client
        .create_table()
        .table_name(table_name)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name(pk_name)
                .attribute_type(pk_type)
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name(pk_name)
                .key_type(KeyType::Hash)
                .build()?,
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await?;

    Ok(true)
}

async fn create_usage_table(client: &aws_sdk_dynamodb::Client, table_name: &str) -> Result<bool> {
    // Check if table already exists
    let tables = client.list_tables().send().await?;
    if tables.table_names().contains(&table_name.to_string()) {
        return Ok(false);
    }

    client
        .create_table()
        .table_name(table_name)
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("api_key")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .attribute_definitions(
            AttributeDefinition::builder()
                .attribute_name("timestamp")
                .attribute_type(ScalarAttributeType::S)
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("api_key")
                .key_type(KeyType::Hash)
                .build()?,
        )
        .key_schema(
            KeySchemaElement::builder()
                .attribute_name("timestamp")
                .key_type(KeyType::Range)
                .build()?,
        )
        .billing_mode(BillingMode::PayPerRequest)
        .send()
        .await?;

    Ok(true)
}
