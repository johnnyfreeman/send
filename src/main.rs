mod cli;
mod executor;
mod logging;
mod masking;
mod request;
mod resolvers;
mod response;

use clap::Parser;
use cli::{Cli, CollectionCommands, Commands, RequestCommands};
use executor::Executor;
use logging::init_logging;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let guard = init_logging()?;

    match Cli::parse().command {
        Commands::Collections { command } => match command {
            CollectionCommands::Run {
                collection,
                output_options,
            } => {
                let requests = request::load_requests_from_toml(collection.as_str())?;

                let cloned_requests: Vec<_> = requests.clone();

                let mut executor = Executor::new(requests);

                for request in cloned_requests {
                    let response = executor.execute_request(request.clone()).await?;

                    executor
                        .render_output(response, output_options.clone())
                        .await?;
                }
            }
        },
        Commands::Requests { command } => match command {
            RequestCommands::Run {
                collection,
                request,
                output_options,
            } => {
                let requests = request::load_requests_from_toml(collection.as_str())?;

                let mut executor = Executor::new(requests);

                let response = executor.execute_request_named(request).await?;

                executor.render_output(response, output_options).await?;
            }
        },
    }

    drop(guard);

    Ok(())
}
