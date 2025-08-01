mod cli;
mod executor;
mod logging;
mod masking;
mod options;
mod request;
mod resolvers;
mod response;
use clap::Parser;
use executor::Executor;
use logging::init_logging;
use options::Options;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guard = init_logging()?;

    let options = Options::parse();

    let requests = request::load_requests_from_toml(options.collection.as_str())?;

    Executor::new(requests, options).execute().await?;

    drop(guard);

    Ok(())
}
