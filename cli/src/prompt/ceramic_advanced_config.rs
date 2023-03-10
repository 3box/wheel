use ceramic_config::*;
use inquire::*;
use std::path::PathBuf;

use crate::did::DidAndPrivateKey;

pub async fn prompt<'a, 'b, Fn, Fut>(
    cfg: &'a mut Config,
    admin_did: &'b DidAndPrivateKey,
    mut func: Fn,
) -> anyhow::Result<()>
where
    Fut: std::future::Future<Output = anyhow::Result<()>>,
    Fn: FnMut(&'a mut Config, &'b DidAndPrivateKey) -> Fut,
{
    let ans = Confirm::new(&format!("Step through ceramic configuration?"))
        .with_help_message("Step through interactive prompts to configure ceramic node")
        .prompt_skippable()?;

    if let Some(true) = ans {
        func(cfg, admin_did).await?;
    }

    Ok(())
}

pub fn configure_ipfs(cfg: &mut Config) -> anyhow::Result<()> {
    let ans = Select::new(
        "Bundled or Remote IPFS (default=Bundled)",
        vec![Ipfs::Bundled, Ipfs::Remote(IpfsRemote::default())],
    )
    .prompt()?;

    let r = if let Ipfs::Remote(_) = ans {
        let ipfs = IpfsRemote {
            host: Text::new("IPFS Hostname").prompt()?,
        };
        Ipfs::Remote(ipfs)
    } else {
        Ipfs::Bundled
    };
    cfg.ipfs = r;
    Ok(())
}

enum StateStoreSelect {
    S3,
    Local,
}

impl std::fmt::Display for StateStoreSelect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::S3 => write!(f, "S3"),
            Self::Local => write!(f, "Local"),
        }
    }
}

pub async fn configure_state_store(cfg: &mut Config) -> anyhow::Result<()> {
    let ans = Select::new(
        "State Store (default=local)",
        vec![StateStoreSelect::Local, StateStoreSelect::S3],
    )
    .prompt()?;

    let r = if let StateStoreSelect::Local = ans {
        let location = Text::new("Directory")
            .with_default("/opt/ceramic/data")
            .prompt()?;
        let location = PathBuf::from(location);
        if !location.exists() {
            if Confirm::new("Directory does not exist, create it?").prompt()? {
                tokio::fs::create_dir_all(location.clone()).await?;
            }
        }
        StateStore::LocalDirectory(location)
    } else {
        let bucket = Text::new("Bucket").with_default("ceramic").prompt()?;
        let endpoint = Text::new("Endpoint").prompt()?;
        StateStore::S3(S3StateStore {
            bucket: bucket,
            endpoint: endpoint,
        })
    };
    cfg.state_store = r;
    Ok(())
}

pub fn configure_http_api(cfg: &mut Config, admin_did: &DidAndPrivateKey) -> anyhow::Result<()> {
    cfg.http_api.hostname = Text::new("Bind address")
        .with_default(&cfg.http_api.hostname)
        .prompt()?;
    cfg.http_api.port = Text::new("Bind port")
        .with_default(&cfg.http_api.port.to_string())
        .prompt()?
        .parse()?;
    let cors = Text::new("Cors origins, comma separated")
        .with_default(&cfg.http_api.cors_allowed_origins.join(","))
        .prompt()?;
    let cors = cors.split(",").map(|s| s.trim().to_string()).collect();
    cfg.http_api.cors_allowed_origins = cors;
    cfg.http_api.admin_dids = vec![admin_did.did().to_string()];
    Ok(())
}

fn configure_network(cfg: &mut Config) -> anyhow::Result<()> {
    cfg.network.id = Select::new(
        "Network type",
        vec![
            NetworkIdentifier::InMemory,
            NetworkIdentifier::Local,
            NetworkIdentifier::Dev,
            NetworkIdentifier::Clay,
            NetworkIdentifier::Mainnet,
        ],
    )
    .with_starting_cursor(3)
    .prompt()?;
    match cfg.network.id {
        NetworkIdentifier::Local | NetworkIdentifier::Dev => {
            let topic = cfg.network.pubsub_topic.clone().unwrap_or_else(|| {
                format!(
                    "/ceramic/local-{}",
                    std::time::Instant::now().elapsed().as_millis()
                )
            });
            let topic = Text::new("Pubsub Topic").with_default(&topic).prompt()?;
            cfg.network.pubsub_topic = Some(topic);
        }
        _ => {
            //doesn't use pubsub topic
        }
    }
    Ok(())
}

fn configure_anchor(cfg: &mut Config) -> anyhow::Result<()> {
    cfg.anchor.anchor_service_url = Text::new("Anchor Service Url")
        .with_default(&cfg.anchor.anchor_service_url)
        .prompt()?;
    cfg.anchor.ethereum_rpc_url = Text::new("Ethereum RPC Url")
        .with_default(&cfg.anchor.ethereum_rpc_url)
        .prompt()?;
    Ok(())
}

pub fn configure_node(cfg: &mut Config) -> anyhow::Result<()> {
    let gateway = Confirm::new("Run as gateway?")
        .with_default(true)
        .prompt()?;
    cfg.node.gateway = gateway;
    Ok(())
}

pub fn configure_indexing(cfg: &mut Config) -> anyhow::Result<()> {
    cfg.indexing.db = Text::new("Database Connection String")
        .with_help_message("Support Postgresql and Sqlite. Sqlite is not allowed on production")
        .with_default(&cfg.indexing.db)
        .prompt()?;
    if !cfg.allows_sqlite() {
        if cfg.indexing.db.starts_with("sqlite") {
            anyhow::bail!("sqlite not allowed in environment {}", cfg.network.id);
        }
    }

    Ok(())
}

pub async fn configure<'a, 'b>(
    cfg: &'a mut Config,
    admin_did: &'b DidAndPrivateKey,
) -> anyhow::Result<()> {
    configure_ipfs(cfg)?;
    configure_state_store(cfg).await?;
    configure_http_api(cfg, admin_did)?;
    configure_network(cfg)?;
    configure_anchor(cfg)?;
    configure_node(cfg)?;
    configure_indexing(cfg)?;

    Ok(())
}
