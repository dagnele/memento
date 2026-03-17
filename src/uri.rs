use anyhow::{Result, bail};

pub const ROOT_URI: &str = "mem://";
pub const RESOURCE_NAMESPACE: &str = "mem://resources";
pub const USER_NAMESPACE: &str = "mem://user";
pub const AGENT_NAMESPACE: &str = "mem://agent";

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum Namespace {
    Resources,
    User,
    Agent,
}

impl Namespace {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resources => "resources",
            Self::User => "user",
            Self::Agent => "agent",
        }
    }

    pub fn root_uri(self) -> &'static str {
        match self {
            Self::Resources => RESOURCE_NAMESPACE,
            Self::User => USER_NAMESPACE,
            Self::Agent => AGENT_NAMESPACE,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ParsedUri {
    Root,
    Namespace(Namespace),
    Item {
        namespace: Namespace,
        relative_path: String,
    },
}

pub fn parse_memento_uri(uri: &str) -> Result<ParsedUri> {
    if uri == ROOT_URI {
        return Ok(ParsedUri::Root);
    }

    let suffix = uri
        .strip_prefix(ROOT_URI)
        .ok_or_else(|| anyhow::anyhow!("unsupported Memento URI: `{uri}`"))?;
    let segments = suffix
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return Ok(ParsedUri::Root);
    }

    let namespace = parse_namespace(segments[0])?;

    match namespace {
        Namespace::Resources => {
            if segments.len() == 1 {
                Ok(ParsedUri::Namespace(namespace))
            } else {
                Ok(ParsedUri::Item {
                    namespace,
                    relative_path: segments[1..].join("/"),
                })
            }
        }
        Namespace::User | Namespace::Agent => {
            if segments.len() == 1 {
                Ok(ParsedUri::Namespace(namespace))
            } else {
                Ok(ParsedUri::Item {
                    namespace,
                    relative_path: segments[1..].join("/"),
                })
            }
        }
    }
}

pub fn build_resource_uri(path: &str) -> String {
    if path.is_empty() {
        RESOURCE_NAMESPACE.to_string()
    } else {
        format!("{RESOURCE_NAMESPACE}/{path}")
    }
}

pub fn build_namespace_item_uri(namespace: Namespace, path: &str) -> String {
    if path.is_empty() {
        namespace.root_uri().to_string()
    } else {
        format!("{}/{path}", namespace.root_uri())
    }
}

fn parse_namespace(segment: &str) -> Result<Namespace> {
    match segment {
        "resources" => Ok(Namespace::Resources),
        "user" => Ok(Namespace::User),
        "agent" => Ok(Namespace::Agent),
        _ => bail!("unsupported Memento URI namespace: `{segment}`"),
    }
}
