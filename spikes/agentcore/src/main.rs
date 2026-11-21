mod core;

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct ProposeArgs {
    #[schemars(description = "hunk ids from the status tool")]
    hunks: Vec<String>,
    #[schemars(description = "commit message")]
    message: String,
    #[schemars(description = "one line on why, shown to the human reviewer")]
    why: String,
}

#[derive(Clone)]
struct Gitgud {
    root: String,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Gitgud {
    #[tool(description = "List uncommitted hunks with stable ids")]
    fn status(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&core::Repo::open(&self.root).hunks()?).map_err(|e| e.to_string())
    }

    #[tool(description = "Propose a commit from hunk ids. Nothing is committed until the human approves it in gitgud.")]
    fn propose(&self, Parameters(a): Parameters<ProposeArgs>) -> Result<String, String> {
        serde_json::to_string_pretty(&core::Repo::open(&self.root).propose(&a.hunks, &a.message, &a.why)?).map_err(|e| e.to_string())
    }

    #[tool(description = "List pending proposals and their risk flags")]
    fn proposals(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&core::Repo::open(&self.root).list()?).map_err(|e| e.to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Gitgud {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions("Stage work as proposals; a human approves them in gitgud.")
    }
}

// usage: agentcore <repo> status | list | propose <msg> <why> <id>... | approve <id> | mcp
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let root = args[1].clone();
    let repo = core::Repo::open(&root);
    match args[2].as_str() {
        "status" => {
            for h in repo.hunks()? {
                println!("{}  {:<50} {} (+{} -{})", h.id, h.path, h.header, h.added, h.removed);
            }
        }
        "list" => println!("{}", serde_json::to_string_pretty(&repo.list()?)?),
        "propose" => println!("{}", serde_json::to_string_pretty(&repo.propose(&args[5..], &args[3], &args[4])?)?),
        "approve" => println!("committed {}", repo.approve(&args[3])?),
        "mcp" => {
            let server = Gitgud { root, tool_router: Gitgud::tool_router() }.serve(rmcp::transport::stdio()).await?;
            server.waiting().await?;
        }
        other => return Err(format!("unknown command {other}").into()),
    }
    Ok(())
}
