use clap::Parser;
use mini_evm::rpc::RpcState;
use mini_evm::rpc::server::app;
use mini_evm::state::StateFork;

#[derive(Debug, Parser)]
#[command(name = "mini-evm-rpc", about = "JSON-RPC server for mini-evm")]
struct Args {
    #[arg(long, default_value_t = 8545)]
    port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", args.port)).await?;
    println!("mini-evm RPC listening on {}", listener.local_addr()?);
    axum::serve(listener, app(RpcState::new(StateFork::default()))).await?;
    Ok(())
}
