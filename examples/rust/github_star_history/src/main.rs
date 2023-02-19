//! Fetch GitHub star history for one or more repositories and log them to Rerun.
//!
//! Code based on <https://github.com/dtolnay/star-history>,
//! with some simplifications and improvements.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    process::ExitCode,
};

use serde::{Deserialize, Serialize};

/// Show number of GitHub stars over time.
#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,

    /// Specifies the repositories or GitHub users to load
    repos: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Error from GitHub api: {0}")]
    GitHub(String),

    #[error("failed to decode response body")]
    DecodeResponse(#[source] serde_json::Error),

    #[error("no such user: {0}")]
    NoSuchUser(String),

    #[error("no such repository: {0}/{1}")]
    NoSuchRepo(String, String),

    #[error(transparent)]
    GhToken(#[from] gh_token::Error),

    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(
        "\
Error: GitHub auth token is not set up.

(Expected config file: {0})

Run `gh auth login` to store a GitHub login token. The `gh` CLI
can be installed from <https://cli.github.com>.

If you prefer not to use the `gh` CLI, you can instead provide
a token through the GITHUB_TOKEN environment variable.
Head to <https://github.com/settings/tokens> and click
\"Generate new token (classic)\". The default public access
permission is sufficient -- you can leave all the checkboxes
empty. Save the generated token somewhere like ~/.githubtoken
and use `export GITHUB_TOKEN=$(cat ~/.githubtoken)`.
"
    )]
    MissingToken(String),
}

type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Series {
    Owner(String),
    Repo(String, String),
}

impl std::fmt::Display for Series {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Series::Owner(owner) => formatter.write_str(owner)?,
            Series::Repo(owner, repo) => {
                formatter.write_str(owner)?;
                formatter.write_str("/")?;
                formatter.write_str(repo)?;
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(transparent)]
struct Cursor(Option<String>);

impl std::fmt::Display for Cursor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(cursor) => {
                formatter.write_str("\"")?;
                formatter.write_str(cursor)?;
                formatter.write_str("\"")?;
            }
            None => formatter.write_str("null")?,
        }
        Ok(())
    }
}

struct Work {
    series: Series,
    cursor: Cursor,
}

#[derive(Serialize)]
struct Request {
    query: String,
}

#[derive(Deserialize, Debug)]
struct Response {
    message: Option<String>,

    #[serde(default, deserialize_with = "deserialize_data")]
    data: VecDeque<Data>,

    #[serde(default)]
    errors: Vec<Message>,
}

#[derive(Deserialize, Debug)]
struct Message {
    message: String,
}

#[derive(Debug)]
enum Data {
    Owner(Option<Owner>),
    Repo(Option<Repo>),
}

#[derive(Deserialize, Debug)]
struct Owner {
    login: String,
    repositories: Repositories,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Repositories {
    page_info: PageInfo,
    nodes: Vec<Repo>,
}

#[derive(Deserialize, Debug)]
struct Repo {
    name: String,
    owner: Account,
    stargazers: Option<Stargazers>,
}

#[derive(Deserialize, Ord, PartialOrd, Eq, PartialEq, Clone, Default, Debug)]
struct Account {
    login: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Stargazers {
    page_info: PageInfo,

    #[serde(deserialize_with = "non_nulls")]
    edges: Vec<Star>,
}

/// Represents a single star-gazer.
#[derive(Deserialize, Ord, PartialOrd, Eq, PartialEq, Clone, Debug)]
struct Star {
    #[serde(rename = "starredAt")]
    time: chrono::DateTime<chrono::Utc>,
    node: Account,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PageInfo {
    has_next_page: bool,
    end_cursor: Cursor,
}

fn main() -> anyhow::Result<ExitCode> {
    re_log::setup_native_logging();

    use clap::Parser as _;
    let args = Args::parse();

    if args.repos.is_empty() {
        eprintln!("You need to specify at least one repository");
        return Ok(ExitCode::FAILURE);
    }

    let mut session = rerun::Session::init("github_star_history", true);

    let should_spawn = args.rerun.on_startup(&mut session);
    if should_spawn {
        session.spawn(move |mut session| run(&mut session, &args))?;
    } else {
        run(&mut session, &args)?;
        args.rerun.on_teardown(&mut session)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn deserialize_data<'de, D>(deserializer: D) -> Result<VecDeque<Data>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, IgnoredAny, MapAccess, Visitor};

    struct ResponseVisitor;

    impl<'de> Visitor<'de> for ResponseVisitor {
        type Value = VecDeque<Data>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("Map<String, Data>")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut data = VecDeque::new();
            while let Some(key) = map.next_key::<String>()? {
                if key.starts_with("owner") {
                    let owner = map.next_value::<Option<Owner>>()?;
                    data.push_back(Data::Owner(owner));
                } else if key.starts_with("repo") {
                    let repo = map.next_value::<Option<Repo>>()?;
                    data.push_back(Data::Repo(repo));
                } else {
                    map.next_value::<IgnoredAny>()?;
                }
            }
            Ok(data)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(VecDeque::new())
        }
    }

    deserializer.deserialize_any(ResponseVisitor)
}

fn non_nulls<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    use serde::de::{SeqAccess, Visitor};

    struct NonNullsVisitor<T>(std::marker::PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for NonNullsVisitor<T>
    where
        T: serde::Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("array")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(next) = seq.next_element::<Option<T>>()? {
                vec.extend(next);
            }
            Ok(vec)
        }
    }

    let visitor = NonNullsVisitor(std::marker::PhantomData);
    deserializer.deserialize_seq(visitor)
}

fn run(session: &mut rerun::Session, args: &Args) -> anyhow::Result<()> {
    let serieses: Vec<Series> = args
        .repos
        .iter()
        .map(|arg| {
            let arg = arg.to_lowercase();
            let mut parts = arg.splitn(2, '/');
            let owner = parts.next().unwrap();
            if let Some(repo) = parts.next() {
                let owner = owner.to_owned();
                let repo = repo.to_owned();
                Series::Repo(owner, repo)
            } else {
                let owner = owner.strip_prefix('@').unwrap_or(owner).to_owned();
                Series::Owner(owner)
            }
        })
        .collect();

    let stars = requests(&serieses)?;

    fn to_rerun_timepoint(time: chrono::DateTime<chrono::Utc>) -> rerun::time::TimePoint {
        let timeline = rerun::time::Timeline::new("time", rerun::time::TimeType::Time);
        let time = rerun::time::Time::from_ns_since_epoch(time.timestamp_nanos());
        [(timeline, time.into())].into()
    }

    for (series, stars) in &stars {
        for (count, star) in stars.iter().enumerate() {
            rerun::MsgSender::new(format!("\"{}\"", series))
                .with_timepoint(to_rerun_timepoint(star.time))
                .with_component(&[rerun::components::Scalar(count as _)])
                .unwrap()
                .send(session)
                .unwrap();
        }
    }

    Ok(())
}

fn requests(serieses: &[Series]) -> Result<BTreeMap<Series, BTreeSet<Star>>> {
    use std::io::Write as _;

    let mut stars = BTreeMap::new();

    let mut work = Vec::new();
    for series in serieses {
        stars.insert(series.clone(), BTreeSet::default());
        work.push(Work {
            series: series.clone(),
            cursor: Cursor(None),
        });
    }

    let authorization = format!("bearer {}", github_token()?.trim());
    let client = reqwest::blocking::Client::new();
    use reqwest::header::{AUTHORIZATION, USER_AGENT};

    eprint!("Fetching");
    while !work.is_empty() {
        let batch_size = work.len().min(50);
        let defer = work.split_off(batch_size);
        let batch = std::mem::replace(&mut work, defer);

        let mut query = String::new();
        query += "{\n";
        for (i, work) in batch.iter().enumerate() {
            let cursor = &work.cursor;
            query += &match &work.series {
                Series::Owner(owner) => query_owner(i, owner, cursor),
                Series::Repo(owner, repo) => query_repo(i, owner, repo, cursor),
            };
        }
        query += "}\n";

        let json = client
            .post("https://api.github.com/graphql")
            .header(USER_AGENT, "rerun/github_star_history")
            .header(AUTHORIZATION, &authorization)
            .json(&Request { query })
            .send()?
            .text()?;

        let response: Response = serde_json::from_str(&json).map_err(Error::DecodeResponse)?;
        if let Some(message) = response.message {
            return Err(Error::GitHub(message));
        }
        for err in response.errors {
            re_log::error!("GitHub: {}", err.message);
        }

        let mut data = response.data;
        let mut queue = batch.into_iter();
        while let Some(node) = data.pop_front() {
            let id = queue.next();
            match node {
                Data::Owner(None) | Data::Repo(None) => match id.unwrap().series {
                    Series::Owner(owner) => return Err(Error::NoSuchUser(owner)),
                    Series::Repo(owner, repo) => return Err(Error::NoSuchRepo(owner, repo)),
                },
                Data::Owner(Some(node)) => {
                    let owner = node.login;
                    for repo in node.repositories.nodes {
                        data.push_back(Data::Repo(Some(repo)));
                    }

                    if node.repositories.page_info.has_next_page {
                        work.push(Work {
                            series: Series::Owner(owner),
                            cursor: node.repositories.page_info.end_cursor,
                        });
                    }
                }
                Data::Repo(Some(node)) => {
                    let owner = node.owner.login;
                    let repo = node.name;

                    if let Some(stargazers) = node.stargazers {
                        let series = Series::Owner(owner.clone());
                        if let Some(owner_stars) = stars.get_mut(&series) {
                            for star in &stargazers.edges {
                                owner_stars.insert(star.clone());
                            }
                        }

                        let series = Series::Repo(owner.clone(), repo.clone());
                        if let Some(repo_stars) = stars.get_mut(&series) {
                            for star in &stargazers.edges {
                                repo_stars.insert(star.clone());
                            }
                        }

                        if stargazers.page_info.has_next_page {
                            work.push(Work {
                                series: Series::Repo(owner, repo),
                                cursor: stargazers.page_info.end_cursor,
                            });
                        }
                    } else {
                        work.push(Work {
                            series: Series::Repo(owner, repo),
                            cursor: Cursor(None),
                        });
                    }
                }
            }
        }

        eprint!(".");
        std::io::stderr().flush().ok();
    }
    eprintln!();

    Ok(stars)
}

fn github_token() -> Result<String> {
    if let Some(token) = std::env::var_os("GITHUB_TOKEN") {
        Ok(token.to_string_lossy().into_owned())
    } else {
        match gh_token::get() {
            Ok(token) => Ok(token),
            Err(gh_token::Error::NotConfigured(path)) => {
                Err(Error::MissingToken(path.to_string_lossy().into()))
            }
            Err(error) => Err(Error::GhToken(error)),
        }
    }
}

fn query_owner(i: usize, login: &str, cursor: &Cursor) -> String {
    r#"
        owner$i: repositoryOwner(login: "$login") {
          login
          repositories(after: $cursor, first: 100, isFork: false, privacy: PUBLIC, ownerAffiliations: [OWNER]) {
            pageInfo {
              hasNextPage
              endCursor
            }
            nodes {
              name
              owner {
                login
              }
            }
          }
        }
    "#
    .replace("$i", &i.to_string())
    .replace("$login", login)
    .replace("$cursor", &cursor.to_string())
}

fn query_repo(i: usize, owner: &str, repo: &str, cursor: &Cursor) -> String {
    r#"
        repo$i: repository(owner: "$owner", name: "$repo") {
          name
          owner {
            login
          }
          stargazers(after: $cursor, first: 100) {
            pageInfo {
              hasNextPage
              endCursor
            }
            edges {
              node {
                login
              }
              starredAt
            }
          }
        }
    "#
    .replace("$i", &i.to_string())
    .replace("$owner", owner)
    .replace("$repo", repo)
    .replace("$cursor", &cursor.to_string())
}
