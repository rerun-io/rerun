use crate::meili::SearchClient;
use crate::{ingest, meili, DEFAULT_INDEX, DEFAULT_KEY, DEFAULT_URL};

use std::io::stdin;
use std::io::stdout;
use std::io::Write as _;
use std::ops::ControlFlow;

/// Simple terminal search client
#[derive(argh::FromArgs)]
#[argh(subcommand, name = "repl")]
pub struct Repl {
    /// name of the meilisearch index to create/query
    #[argh(positional, default = "DEFAULT_INDEX.into()")]
    index_name: String,

    #[argh(
        option,
        default = "false",
        description = "ingest before starting the repl"
    )]
    ingest: bool,

    /// meilisearch URL
    #[argh(option, long = "url", default = "DEFAULT_URL.into()")]
    meilisearch_url: String,

    /// meilisearch master key (must support both read and write)
    #[argh(option, long = "master-key", default = "DEFAULT_KEY.into()")]
    meilisearch_master_key: String,
}

impl Repl {
    pub fn run(self) -> anyhow::Result<()> {
        let client = meili::connect(&self.meilisearch_url, &self.meilisearch_master_key)?;

        if self.ingest {
            let documents = ingest::run()?;
            client.index(&self.index_name, &documents)?;
        }

        let mut lines = stdin().lines();
        loop {
            stdout().write_all(b"\n> ").unwrap();
            stdout().flush().unwrap();

            match lines.next().transpose()? {
                Some(line) => match self.handle_line(&client, &line)? {
                    ControlFlow::Continue(_) => continue,
                    ControlFlow::Break(_) => break Ok(()),
                },
                None => break Ok(()),
            }
        }
    }

    fn handle_line(&self, search: &SearchClient, line: &str) -> anyhow::Result<ControlFlow<()>> {
        let line = line.trim();
        match line {
            "quit" | "q" | "" => return Ok(ControlFlow::Break(())),
            "reindex" => {
                let documents = ingest::run()?;
                search.index(&self.index_name, &documents)?;
            }
            _ => {
                for result in search.query(&self.index_name, line, Some(4))? {
                    let content = result
                        .content()
                        .split('\n')
                        .map(|line| format!("   {line}"))
                        .collect::<Vec<_>>()
                        .join("\n");

                    println!("### {} [{}]", result.title(), result.url(),);
                    if content.len() > 200 {
                        println!("{}…\n", &content[..200]);
                    } else {
                        println!("{content}\n");
                    }
                }
            }
        }
        Ok(ControlFlow::Continue(()))
    }
}
