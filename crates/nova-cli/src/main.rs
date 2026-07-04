use clap::Parser;
use nova_cli::app::{Cli, CommandContext};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let ctx = CommandContext {
        address: cli.address,
        api_key: cli.api_key,
        output: cli.output,
    };
    cli.command.execute(&ctx)
}
