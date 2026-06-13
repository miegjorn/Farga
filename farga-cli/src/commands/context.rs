use clap::Subcommand;
use farga_core::reader::{HttpFargaReader, FargaReader};

#[derive(Subcommand)]
pub enum ContextKind {
    Org { org: String },
    Project { project: String },
    Component { project: String, path: String },
}

pub async fn run(base: &str, kind: ContextKind) -> anyhow::Result<()> {
    let reader = HttpFargaReader::new(base.to_string());
    match kind {
        ContextKind::Org { org } => {
            let ctx = reader.org_layer(&org).await?;
            println!("{}", ctx.content);
        }
        ContextKind::Project { project } => {
            let ctx = reader.project_layer(&project).await?;
            println!("{}", ctx.content);
        }
        ContextKind::Component { project, path } => {
            let ctx = reader.component_layer(&project, &path).await?;
            println!("{}", ctx.content);
        }
    }
    Ok(())
}
