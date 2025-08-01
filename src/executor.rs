use crate::cli::OutputOptions;
use crate::masking::mask_json;
use crate::request::{Dependencies, Dependency, Request, RequestBody};
use crate::resolvers::env_var_resolver::EnvVarResolver;
use crate::resolvers::one_password_resolver::{OnePasswordResolver, OnePasswordResolverError};
use crate::resolvers::prompt_resolver::{PromptResolver, PromptResolverError};
use crate::resolvers::response_resolver::ResponseResolver;
use crate::resolvers::Resolver;
use crate::response::Response;
use bat::PrettyPrinter;
use console::style;
use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, InvalidHeaderValue};
use reqwest::Client;
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;
use tracing::{debug, error};

lazy_static! {
    static ref PLACEHOLDER_REGEX: Regex = Regex::new(r"\{(\w+)\}").unwrap();
}

#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Request `{request:?}` was not found in history")]
    RequestNotFound { request: String },
    #[error(transparent)]
    DependencyResolutionFailed(#[from] DependencyResolutionError),
    #[error("Unknown error: `{0:?}`")]
    Unknown(String),
}

#[derive(Error, Debug)]
pub enum DependencyResolutionError {
    #[error(transparent)]
    OnePasswordDependencyFailed(#[from] OnePasswordResolverError),
    #[error(transparent)]
    PromptDependencyFailed(#[from] PromptResolverError),
    #[error("Not yet implemented: `{0}`")]
    NotImplemented(String),
    #[error("Dependency definition for `{placeholder:?}` could not be found")]
    PlaceholderDefinitionNotFound { placeholder: String },
}

// Cache for loaded env files
static ENV_FILES_CACHE: Lazy<Mutex<HashMap<String, HashMap<String, String>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug)]
pub struct Executor {
    requests: HashMap<String, Request>,
    http: Client,
    env_var_resolver: EnvVarResolver,
    prompt_resolver: PromptResolver,
    response_resolver: ResponseResolver,
    one_password_resolver: OnePasswordResolver,
}

impl Executor {
    pub fn new(requests: Vec<Request>) -> Self {
        Self {
            requests: requests
                .into_iter()
                .map(|request| (request.name.clone(), request))
                .collect(),
            http: Client::new(),
            env_var_resolver: EnvVarResolver::new(),
            prompt_resolver: PromptResolver::new(),
            response_resolver: ResponseResolver::new(),
            one_password_resolver: OnePasswordResolver::new(),
        }
    }

    pub async fn execute_request_named(
        &mut self,
        request: String,
    ) -> Result<Response, ExecutionError> {
        let request = self
            .requests
            .get(&request)
            .cloned()
            .ok_or_else(|| ExecutionError::RequestNotFound { request })?;

        self.execute_request(request).await
    }

    pub async fn execute_request(&mut self, request: Request) -> Result<Response, ExecutionError> {
        // Resolve URL
        let url = self
            .resolve_placeholders(&request.url, request.dependencies.as_ref())
            .await?;
        debug!(url);

        let headers = if let Some(header_map) = &request.headers {
            let mut resolved_headers = HeaderMap::new();
            for (key, value) in header_map {
                let resolved_key = self
                    .resolve_placeholders(key, request.dependencies.as_ref())
                    .await
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                let resolved_value = self
                    .resolve_placeholders(value, request.dependencies.as_ref())
                    .await
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                let header_name = HeaderName::from_bytes(resolved_key.as_bytes())
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                resolved_headers.insert(
                    header_name,
                    resolved_value
                        .parse()
                        .map_err(|error: InvalidHeaderValue| {
                            ExecutionError::Unknown(error.to_string())
                        })?,
                );
            }
            resolved_headers
        } else {
            HeaderMap::new()
        };
        debug!("{:?}", headers);

        // Resolve the request body, if it exists
        let body = match &request.body {
            Some(RequestBody::Text(text)) => Some(RequestBody::Text(
                self.resolve_placeholders(text, request.dependencies.as_ref())
                    .await
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?,
            )),
            Some(RequestBody::Json(json)) => {
                // Serialize the entire JSON value to a string
                let json_string = serde_json::to_string(&json)
                    .map_err(|e| ExecutionError::Unknown(format!("Serialization error: {}", e)))?;

                // Apply the resolve_placeholders function
                let resolved_string = self
                    .resolve_placeholders(&json_string, request.dependencies.as_ref())
                    .await
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?;

                // Deserialize the resolved string back into a JSON value
                let resolved_value = serde_json::from_str(&resolved_string).map_err(|e| {
                    ExecutionError::Unknown(format!("Deserialization error: {}", e))
                })?;

                Some(RequestBody::Json(resolved_value))
            }
            Some(RequestBody::Form(hash_map)) => {
                let mut resolved_form = HashMap::new();
                for (key, value) in hash_map {
                    let resolved_value = self
                        .resolve_placeholders(value, request.dependencies.as_ref())
                        .await
                        .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                    resolved_form.insert(key.clone(), resolved_value);
                }
                Some(RequestBody::Form(resolved_form))
            }
            None => None,
        };
        debug!("{:?}", body);

        // Execute the request and capture the response
        let response = {
            let builder = self
                .http
                .request(
                    reqwest::Method::from_bytes(request.method.as_bytes())
                        .map_err(|error| ExecutionError::Unknown(error.to_string()))?,
                    &url,
                )
                .headers(headers);

            let builder = match body {
                Some(RequestBody::Text(text)) => builder.body(text),
                Some(RequestBody::Json(json)) => builder.json(&json),
                Some(RequestBody::Form(form)) => builder.form(&form),
                None => builder,
            };
            debug!("{:?}", builder);

            builder
                .send()
                .await
                .map_err(|error| ExecutionError::Unknown(error.to_string()))?
        };

        let response = Response {
            request: request.clone(),
            headers: response.headers().clone(),
            status: response.status(),
            text: response
                .text()
                .await
                .map_err(|error| ExecutionError::Unknown(error.to_string()))?,
        };
        debug!("{:?}", response);

        self.response_resolver.save_to_history(response.clone());

        Ok(response)
    }

    pub async fn render_output(
        &mut self,
        response: Response,
        options: OutputOptions,
    ) -> Result<(), ExecutionError> {
        if !options.hide_status {
            if options.raw_output {
                println!(
                    "{}: {}",
                    response.status.as_str(),
                    response.status.canonical_reason().unwrap_or(""),
                );
            } else {
                println!(
                    "{} {}",
                    if response.status.is_client_error() {
                        style(format!(" {} ", response.status.as_str()))
                            .on_yellow()
                            .black()
                    } else if response.status.is_server_error() {
                        style(format!(" {} ", response.status.as_str()))
                            .on_red()
                            .black()
                    } else {
                        style(format!(" {} ", response.status.as_str()))
                            .on_green()
                            .black()
                    },
                    style(response.status.canonical_reason().unwrap_or("")).bold(),
                );
            }
        }

        if options.show_headers {
            let mut headers = response.headers.clone();

            if !options.disable_masking {
                for (_key, value) in &mut headers {
                    if let Some(value_str) = value.to_str().ok() {
                        let masked_value = mask_json(
                            serde_json::json!(value_str),
                            &response.request.masking_rules,
                        )
                        .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                        *value = HeaderValue::from_str(masked_value.as_str().unwrap())
                            .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                    }
                }
            }

            if options.raw_output {
                for (key, value) in headers {
                    println!(
                        "{}: {}",
                        key.as_ref().map(|k| k.as_str()).unwrap_or(""),
                        value.to_str().unwrap_or("")
                    );
                }
            } else {
                let mut headers_formatted = String::new();
                for (key, value) in headers {
                    let key_str = key.as_ref().map(|k| k.as_str()).unwrap_or("");
                    let value_str = value.to_str().unwrap_or("");
                    headers_formatted.push_str(&format!("{}: {}\n", key_str, value_str));
                }

                PrettyPrinter::new()
                    .input_from_bytes(headers_formatted.as_bytes())
                    .language("toml")
                    .print()
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
            }
        }

        if !options.hide_body {
            let mut body = serde_json::from_str::<serde_json::Value>(&response.text)
                .map_err(|error| ExecutionError::Unknown(error.to_string()))?;

            if !options.disable_masking {
                body = mask_json(body, &response.request.masking_rules)
                    .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
            }

            if options.raw_output {
                println!(
                    "{}",
                    serde_json::to_string(&body)
                        .map_err(|error| ExecutionError::Unknown(error.to_string()))?
                );
            } else {
                if let Ok(pretty_json) = serde_json::to_string_pretty(&body) {
                    PrettyPrinter::new()
                        .input_from_bytes(pretty_json.as_bytes())
                        .language("json")
                        .print()
                        .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                } else {
                    PrettyPrinter::new()
                        .input_from_bytes(response.text.as_bytes())
                        .language("plain")
                        .print()
                        .map_err(|error| ExecutionError::Unknown(error.to_string()))?;
                }
            }
        }

        Ok(())
    }

    async fn resolve_placeholders(
        &mut self,
        template: &str,
        request_dependencies: Option<&Dependencies>,
    ) -> Result<String, DependencyResolutionError> {
        let mut resolved = template.to_string();

        // Find all placeholders in the template
        for caps in PLACEHOLDER_REGEX.captures_iter(template) {
            let placeholder = &caps[1]; // The name inside {}

            // Try to resolve the placeholder
            let value = if let Some(dep) = request_dependencies
                .as_ref()
                .and_then(|deps| deps.get(placeholder))
            {
                self.resolve_dependency_value(dep, placeholder).await?
            } else {
                error!("Resolving {} from request {}", placeholder, template);
                return Err(DependencyResolutionError::PlaceholderDefinitionNotFound {
                    placeholder: placeholder.to_string(),
                });
            };

            // Replace the placeholder in the resolved string
            resolved = resolved.replace(&format!("{{{}}}", placeholder), &value);
        }

        Ok(resolved)
    }

    async fn resolve_dependency_value(
        &mut self,
        dep: &Dependency,
        _placeholder: &str,
    ) -> Result<String, DependencyResolutionError> {
        match dep {
            Dependency::EnvFile {
                env_file,
                key,
                prompt,
            } => {
                // Load the TOML file
                let mut env_data = load_env_file(env_file).map_err(|error| {
                    DependencyResolutionError::NotImplemented(error.to_string())
                })?;

                if let Some(value) = env_data.get(key) {
                    Ok(value.clone())
                } else if let Some(prompt) = prompt {
                    // Prompt the user
                    let value = self.prompt_resolver.resolve(prompt.clone())?;
                    // Optionally, save the value back to the env file or cache
                    env_data.insert(key.clone(), value.clone());
                    // Save back to the file
                    save_env_file(env_file, &env_data).map_err(|error| {
                        DependencyResolutionError::NotImplemented(error.to_string())
                    })?;
                    // Update the cache
                    let mut cache = ENV_FILES_CACHE.lock().unwrap();
                    cache.insert(env_file.clone(), env_data);
                    Ok(value)
                } else {
                    Err(DependencyResolutionError::NotImplemented(format!(
                        "Could resolve {} key from env file {}",
                        key, env_file
                    )))
                }
            }
            Dependency::EnvVar { name, prompt } => {
                if let Ok(env_value) = self
                    .env_var_resolver
                    .resolve((name.to_owned(), prompt.clone()))
                {
                    Ok(env_value)
                } else {
                    Err(DependencyResolutionError::NotImplemented(format!(
                        "Could resolve variable {} from env",
                        name
                    )))
                }
            }
            Dependency::OnePassword { vault, item, field } => Ok(self
                .one_password_resolver
                .resolve((vault.clone(), item.clone(), field.clone()))?),
            Dependency::File { path } => {
                let file_content = std::fs::read_to_string(path).map_err(|error| {
                    DependencyResolutionError::NotImplemented(error.to_string())
                })?;
                Ok(file_content.trim().to_string())
            }
            Dependency::Response { request, target } => {
                // Check if the request is already resolved
                if let Ok(value) = self
                    .response_resolver
                    .resolve((request.clone(), target.clone()))
                {
                    return Ok(value);
                }

                let cloned_request = self
                    .requests
                    .get(request)
                    .ok_or(DependencyResolutionError::NotImplemented(format!(
                        "Request configuration for {} could not be found",
                        request
                    )))?
                    .clone();

                Box::pin(self.execute_request(cloned_request))
                    .await
                    .map_err(|error| {
                        DependencyResolutionError::NotImplemented(error.to_string())
                    })?;

                Ok(self
                    .response_resolver
                    .resolve((request.to_owned(), target.to_owned()))
                    .map_err(|error| {
                        DependencyResolutionError::NotImplemented(error.to_string())
                    })?)
            }
            Dependency::Prompt { label } => Ok(self.prompt_resolver.resolve(label.clone())?),
        }
    }
}

fn load_env_file(env_file: &str) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut cache = ENV_FILES_CACHE.lock().unwrap();

    if let Some(data) = cache.get(env_file) {
        Ok(data.clone())
    } else {
        // Read and parse the TOML file
        let content = std::fs::read_to_string(env_file)?;
        let data: HashMap<String, String> = toml::from_str(&content)?;
        cache.insert(env_file.to_string(), data.clone());
        Ok(data)
    }
}

fn save_env_file(
    env_file: &str,
    data: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = toml::to_string(data)?;
    std::fs::write(env_file, content)?;
    Ok(())
}
