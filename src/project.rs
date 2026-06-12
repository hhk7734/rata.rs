use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use openapiv3::{OpenAPI, Operation as OpenApiOperation, PathItem, ReferenceOr};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    pub fn label(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DEL",
        }
    }

    pub fn directory(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Put => "put",
            Self::Patch => "patch",
            Self::Delete => "delete",
        }
    }

    pub fn reqwest(self) -> reqwest::Method {
        match self {
            Self::Get => reqwest::Method::GET,
            Self::Post => reqwest::Method::POST,
            Self::Put => reqwest::Method::PUT,
            Self::Patch => reqwest::Method::PATCH,
            Self::Delete => reqwest::Method::DELETE,
        }
    }
}

impl FromStr for HttpMethod {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" | "DEL" => Ok(Self::Delete),
            other => anyhow::bail!("unsupported HTTP method: {other}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationParameter {
    pub name: String,
    pub location: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub method: HttpMethod,
    pub path: String,
    pub summary: String,
    pub tag: String,
    pub parameters: Vec<OperationParameter>,
}

#[derive(Debug, Clone)]
pub struct Collection {
    pub name: String,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Clone)]
pub struct MatchedOperation {
    pub operation: Operation,
    pub path_params: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct ExampleFile {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, serde::Deserialize)]
pub struct ExampleData {
    pub params: Option<HashMap<String, String>>,
    pub headers: Option<HashMap<String, String>>,
    pub body: Option<serde_json::Value>,
}

impl ExampleFile {
    pub fn load_data(&self) -> anyhow::Result<ExampleData> {
        let content = fs::read_to_string(&self.path)?;
        let data: ExampleData = serde_yaml::from_str(&content)?;
        Ok(data)
    }
}

#[derive(Debug, Clone)]
pub struct RataProject {
    root: PathBuf,
    openapi_path: PathBuf,
    server_url: Option<String>,
    collections: Vec<Collection>,
}

impl RataProject {
    pub fn discover(start: impl AsRef<Path>) -> anyhow::Result<Option<Self>> {
        let Some(rata_dir) = find_rata_dir(start.as_ref()) else {
            return Ok(None);
        };
        let Some(openapi_path) = find_openapi_file(&rata_dir) else {
            return Ok(None);
        };

        let source = fs::read_to_string(&openapi_path)?;
        let document: OpenAPI =
            if openapi_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                serde_json::from_str(&source)?
            } else {
                serde_yaml::from_str(&source)?
            };

        let collections = collect_operations(&document);

        Ok(Some(Self {
            root: rata_dir,
            openapi_path,
            server_url: document.servers.first().map(|server| server.url.clone()),
            collections,
        }))
    }

    pub fn openapi_path(&self) -> &Path {
        &self.openapi_path
    }

    pub fn collections(&self) -> &[Collection] {
        &self.collections
    }

    pub fn server_url(&self) -> Option<&str> {
        self.server_url.as_deref()
    }

    pub fn match_url(
        &self,
        method: HttpMethod,
        url: &str,
    ) -> anyhow::Result<Option<MatchedOperation>> {
        let url = Url::parse(url)?;
        let actual_path = url.path();

        for operation in self
            .collections
            .iter()
            .flat_map(|collection| &collection.operations)
        {
            if operation.method != method {
                continue;
            }

            if let Some(path_params) = match_path_template(&operation.path, actual_path) {
                return Ok(Some(MatchedOperation {
                    operation: operation.clone(),
                    path_params,
                }));
            }
        }

        Ok(None)
    }

    pub fn examples_for(&self, operation: &Operation) -> anyhow::Result<Vec<ExampleFile>> {
        let path = operation.path.trim_start_matches('/');
        let example_dir = self.root.join("examples").join(path).join(operation.method.directory());
        if !example_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut examples = Vec::new();
        for entry in fs::read_dir(example_dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_yaml = matches!(
                path.extension().and_then(|extension| extension.to_str()),
                Some("yaml" | "yml")
            );
            if !is_yaml {
                continue;
            }

            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            examples.push(ExampleFile {
                name: name.to_string(),
                path,
            });
        }
        examples.sort_by(|left, right| left.name.cmp(&right.name));

        Ok(examples)
    }
}

fn find_rata_dir(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?.to_path_buf()
    } else {
        start.to_path_buf()
    };

    loop {
        let candidate = current.join(".rata");
        if candidate.is_dir() {
            return Some(candidate);
        }

        if !current.pop() {
            return None;
        }
    }
}

fn find_openapi_file(rata_dir: &Path) -> Option<PathBuf> {
    ["openapi.yaml", "openapi.yml", "openapi.json"]
        .into_iter()
        .map(|name| rata_dir.join(name))
        .find(|path| path.is_file())
}

fn collect_operations(document: &OpenAPI) -> Vec<Collection> {
    let mut collections = Vec::<Collection>::new();
    let mut collection_indexes = HashMap::<String, usize>::new();

    for (path, item) in &document.paths.paths {
        let ReferenceOr::Item(item) = item else {
            continue;
        };

        for (method, operation) in operations_for_path(item) {
            let tag = operation
                .tags
                .first()
                .cloned()
                .unwrap_or_else(|| "default".to_string());
            let summary = operation
                .summary
                .clone()
                .or_else(|| operation.operation_id.clone())
                .unwrap_or_else(|| path.clone());

            let mut parameters = Vec::new();
            for param in item.parameters.iter().chain(operation.parameters.iter()) {
                if let ReferenceOr::Item(param) = param {
                    let (data, location) = match param {
                        openapiv3::Parameter::Query { parameter_data, .. } => (parameter_data, "query"),
                        openapiv3::Parameter::Header { parameter_data, .. } => (parameter_data, "header"),
                        openapiv3::Parameter::Path { parameter_data, .. } => (parameter_data, "path"),
                        openapiv3::Parameter::Cookie { parameter_data, .. } => (parameter_data, "cookie"),
                    };
                    parameters.push(OperationParameter {
                        name: data.name.clone(),
                        location: location.to_string(),
                        description: data.description.clone(),
                        required: data.required,
                    });
                }
            }

            let operation = Operation {
                method,
                path: path.clone(),
                summary,
                tag,
                parameters,
            };

            let index = *collection_indexes
                .entry(operation.tag.clone())
                .or_insert_with(|| {
                    collections.push(Collection {
                        name: operation.tag.clone(),
                        operations: Vec::new(),
                    });
                    collections.len() - 1
                });
            collections[index].operations.push(operation);
        }
    }

    collections
}

fn operations_for_path(item: &PathItem) -> impl Iterator<Item = (HttpMethod, &OpenApiOperation)> {
    [
        (HttpMethod::Get, item.get.as_ref()),
        (HttpMethod::Post, item.post.as_ref()),
        (HttpMethod::Put, item.put.as_ref()),
        (HttpMethod::Patch, item.patch.as_ref()),
        (HttpMethod::Delete, item.delete.as_ref()),
    ]
    .into_iter()
    .filter_map(|(method, operation)| operation.map(|operation| (method, operation)))
}

fn match_path_template(template: &str, actual: &str) -> Option<Vec<(String, String)>> {
    let template_segments = segments(template);
    let actual_segments = segments(actual);
    if template_segments.len() != actual_segments.len() {
        return None;
    }

    let mut params = Vec::new();
    for (template_segment, actual_segment) in template_segments.iter().zip(actual_segments) {
        if let Some(name) = template_segment
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        {
            params.push((name.to_string(), actual_segment.to_string()));
            continue;
        }

        if *template_segment != actual_segment {
            return None;
        }
    }

    Some(params)
}

fn segments(path: &str) -> Vec<&str> {
    path.trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}
