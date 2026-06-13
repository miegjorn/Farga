use clap::Subcommand;
#[derive(Subcommand)]
pub enum ProposalAction { List, Trigger }
pub async fn run(_base: &str, _action: ProposalAction) -> anyhow::Result<()> {
    println!("proposals: not yet implemented (optimizer agent scheduled for v0.2.0)");
    Ok(())
}
