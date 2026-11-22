use crate::models::AppErrorKind;
use crate::models::{AppError, YamlEncoding, YamlViewMode};
use k8s_openapi::{ClusterResourceScope, NamespaceResourceScope};
use kube::{
    Client,
    api::{Api, Resource},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::fmt::Write as _;

pub fn redact_secret(secret: &mut k8s_openapi::api::core::v1::Secret) {
    redact_secret_metadata(&mut secret.metadata);
    if let Some(ref mut data) = secret.data {
        for value in data.values_mut() {
            *value = k8s_openapi::ByteString(b"REDACTED".to_vec());
        }
    }
    if let Some(ref mut string_data) = secret.string_data {
        for value in string_data.values_mut() {
            *value = "REDACTED".to_string();
        }
    }
}

pub fn redact_secret_metadata(metadata: &mut kube::api::ObjectMeta) {
    if let Some(annotations) = &mut metadata.annotations {
        // kubectl stores the complete previous document, including Secret data.
        if let Some(value) = annotations.get_mut("kubectl.kubernetes.io/last-applied-configuration")
        {
            *value = "REDACTED".into();
        }
    }
}

pub fn normalize_k8s_yaml_value(value: &mut Value, mode: YamlViewMode) {
    if let Value::Object(root) = value {
        if let Some(Value::Object(metadata)) = root.get_mut("metadata") {
            metadata.remove("managedFields");
            if matches!(mode, YamlViewMode::ApplyClean) {
                metadata.remove("uid");
                metadata.remove("resourceVersion");
                metadata.remove("generation");
                metadata.remove("creationTimestamp");
                metadata.remove("selfLink");
            }
        }

        if matches!(mode, YamlViewMode::ApplyClean) {
            root.remove("status");
        }
    }
}

pub fn serialize_resource_document<T: Serialize>(
    resource: &T,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    serialize_resource_value(resource_json_value(resource)?, mode, encoding)
}

pub fn resource_json_value<T: Serialize>(resource: &T) -> Result<Value, AppError> {
    serde_json::to_value(resource)
        .map_err(|e| AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e))
}

#[inline(never)]
pub fn serialize_resource_value(
    mut value: Value,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    normalize_k8s_yaml_value(&mut value, mode);
    serialize_json_value_document(&value, encoding)
}

pub fn serialize_json_value_document(
    value: &Value,
    encoding: YamlEncoding,
) -> Result<String, AppError> {
    match encoding {
        YamlEncoding::Yaml => serde_yaml::to_string(value)
            .map_err(|e| AppError::new(e.to_string(), AppErrorKind::Serialization).with_source(e)),
        YamlEncoding::Kyaml => {
            let mut output = String::new();
            write_kyaml_value(&mut output, value, 0);
            output.push('\n');
            Ok(output)
        }
    }
}

fn write_kyaml_value(output: &mut String, value: &Value, indent: usize) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => {
            let _ = write!(output, "{value}");
        }
        Value::String(value) => write_kyaml_string(output, value),
        Value::Array(values) => write_kyaml_array(output, values, indent),
        Value::Object(map) => write_kyaml_object(output, map, indent),
    }
}

fn write_kyaml_array(output: &mut String, values: &[Value], indent: usize) {
    output.push('[');
    let child_indent = indent + 2;
    for value in values {
        write_kyaml_indent(output, child_indent);
        write_kyaml_value(output, value, child_indent);
        output.push(',');
    }
    if !values.is_empty() {
        write_kyaml_indent(output, indent);
    }
    output.push(']');
}

fn write_kyaml_object(output: &mut String, map: &serde_json::Map<String, Value>, indent: usize) {
    output.push('{');
    let child_indent = indent + 2;
    for (key, value) in map {
        write_kyaml_indent(output, child_indent);
        if is_plain_kyaml_key(key) {
            output.push_str(key);
        } else {
            write_kyaml_string(output, key);
        }
        output.push_str(": ");
        write_kyaml_value(output, value, child_indent);
        output.push(',');
    }
    if !map.is_empty() {
        write_kyaml_indent(output, indent);
    }
    output.push('}');
}

fn write_kyaml_indent(output: &mut String, indent: usize) {
    output.push('\n');
    output.extend(std::iter::repeat_n(' ', indent));
}

fn write_kyaml_string(output: &mut String, value: &str) {
    output.push('"');
    let mut start = 0;
    for (index, byte) in value.bytes().enumerate() {
        let escape = match byte {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\x08' => "\\b",
            b'\t' => "\\t",
            b'\n' => "\\n",
            b'\x0c' => "\\f",
            b'\r' => "\\r",
            byte if byte < b' ' => {
                output.push_str(&value[start..index]);
                let _ = write!(output, "\\u{byte:04x}");
                start = index + 1;
                continue;
            }
            _ => continue,
        };
        // Only ASCII bytes are escaped, so these offsets are UTF-8 boundaries.
        output.push_str(&value[start..index]);
        output.push_str(escape);
        start = index + 1;
    }
    output.push_str(&value[start..]);
    output.push('"');
}

fn is_plain_kyaml_key(key: &str) -> bool {
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/'))
}

pub async fn fetch_and_serialize<
    T: Resource<Scope = NamespaceResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    namespace: Option<&str>,
    name: &str,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    fetch_and_serialize_with_encoding(
        client,
        namespace,
        name,
        YamlViewMode::Kubectl,
        YamlEncoding::Yaml,
    )
    .await
}

pub async fn fetch_and_serialize_with_encoding<
    T: Resource<Scope = NamespaceResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    namespace: Option<&str>,
    name: &str,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    let resource = fetch_resource(client, namespace, name).await?;
    let yaml = serialize_resource_document(&resource, mode, encoding)?;
    Ok((resource, yaml))
}

pub async fn fetch_resource<
    T: Resource<Scope = NamespaceResourceScope>
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    namespace: Option<&str>,
    name: &str,
) -> Result<T, AppError>
where
    <T as Resource>::DynamicType: Default,
{
    super::validate_namespace(namespace)?;
    super::validate_path_segment(name, "resource name")?;
    let api: Api<T> = if let Some(ns) = namespace {
        Api::namespaced(client, ns)
    } else {
        return Err(AppError::new(
            "namespace is required for namespaced resources",
            AppErrorKind::Validation,
        ));
    };
    api.get(name).await.map_err(AppError::from)
}

pub async fn fetch_and_serialize_cluster<
    T: Resource<Scope = ClusterResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    name: &str,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    fetch_and_serialize_cluster_with_encoding(
        client,
        name,
        YamlViewMode::Kubectl,
        YamlEncoding::Yaml,
    )
    .await
}

pub async fn fetch_and_serialize_cluster_with_encoding<
    T: Resource<Scope = ClusterResourceScope>
        + Serialize
        + DeserializeOwned
        + Clone
        + std::fmt::Debug
        + Send
        + Sync,
>(
    client: Client,
    name: &str,
    mode: YamlViewMode,
    encoding: YamlEncoding,
) -> Result<(T, String), AppError>
where
    <T as Resource>::DynamicType: Default,
{
    let resource = fetch_cluster_resource(client, name).await?;
    let yaml = serialize_resource_document(&resource, mode, encoding)?;
    Ok((resource, yaml))
}

pub async fn fetch_cluster_resource<
    T: Resource<Scope = ClusterResourceScope> + DeserializeOwned + Clone + std::fmt::Debug,
>(
    client: Client,
    name: &str,
) -> Result<T, AppError>
where
    <T as Resource>::DynamicType: Default,
{
    super::validate_path_segment(name, "resource name")?;
    let api: Api<T> = Api::all(client);
    api.get(name).await.map_err(AppError::from)
}

#[cfg(test)]
#[path = "serialization_tests.rs"]
mod tests;
