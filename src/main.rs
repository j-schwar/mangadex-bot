use clap::{command, Parser};
use db::MongoClient;

mod db;
mod discord;
mod mangadex;
mod scan;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The token used to authenticate this discord bot.
    #[arg(long, env = "MANGADEX_BOT_DISCORD_TOKEN")]
    discord_token: String,

    /// Id of the guild to add application commands to.
    ///
    /// If not specified, application commands will be registered globally for
    /// all guilds. It may take upwards of an hour for discord to recognize
    /// these commands thus it is best to specify a guild id when testing.
    #[arg(long, env = "MANGADEX_BOT_GUILD_ID")]
    guild_id: Option<u64>,

    /// The connection string of the Azure Cosmos DB for MongoDB account.
    #[arg(long, env = "MANGADEX_BOT_CONNECTION_STRING")]
    connection_string: String,

    /// The name of the database within the account.
    #[arg(long, env = "MANGADEX_BOT_DATABASE")]
    database: String,

    /// The name of the collection within the database.
    #[arg(long, env = "MANGADEX_BOT_COLLECTION")]
    collection: String,

    /// The period between scans in seconds (default 6 hours).
    #[arg(long, env = "MANGADEX_BOT_SCAN_PERIOD", default_value = "21600")]
    scan_period: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    let db_client =
        MongoClient::connect(&args.connection_string, &args.database, &args.collection).await?;
    let commands = discord::command::init(&args, db_client.clone());
    let mut client = discord::init(
        &args.discord_token,
        args.guild_id,
        args.scan_period,
        db_client,
        commands,
    )
    .await?;
    client.start().await?;

    Ok(())
}
