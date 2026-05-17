#[tokio::main]
async fn main() -> anyhow::Result<()> {
    sm_cli::run().await
}
