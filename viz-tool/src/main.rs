use clap::Parser;
use viz_tool::cli::Cli;
use viz_tool::cli_runner::run_cli;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    run_cli(cli)
}
