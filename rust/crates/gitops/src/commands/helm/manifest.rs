use crate::models::{HelmManifestResourceSummary, HelmManifestSummary};
use serde::de::{
    DeserializeSeed, Deserializer, EnumAccess, Error as _, IgnoredAny, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};
use std::{collections::HashSet, fmt};

#[cfg(test)]
pub(super) fn manifest_summary(
    manifest: Option<&str>,
    _fallback_namespace: Option<&str>,
) -> HelmManifestSummary {
    manifest_summary_from_resources(&manifest_resources(manifest))
}

pub fn manifest_summary_from_resources(
    manifest_resources: &[HelmManifestResourceSummary],
) -> HelmManifestSummary {
    const RESOURCE_PREVIEW_LIMIT: usize = 50;

    HelmManifestSummary {
        resource_count: manifest_resources.len(),
        resources: manifest_resources
            .iter()
            .take(RESOURCE_PREVIEW_LIMIT)
            .cloned()
            .collect(),
        truncated: manifest_resources.len() > RESOURCE_PREVIEW_LIMIT,
    }
}

pub fn manifest_resources(manifest: Option<&str>) -> Vec<HelmManifestResourceSummary> {
    let Some(manifest) = manifest.filter(|value| !value.trim().is_empty()) else {
        return Vec::new();
    };

    let mut resources = Vec::new();
    for document in serde_yaml::Deserializer::from_str(manifest) {
        let mut resource = HelmManifestResourceSummary {
            api_version: None,
            kind: None,
            name: None,
            namespace: None,
        };
        if Projection::Manifest(&mut resource)
            .deserialize(document)
            .is_err()
        {
            // The iterator cannot advance past a YAML syntax error.
            break;
        }
        if resource.api_version.is_some() || resource.kind.is_some() || resource.name.is_some() {
            resources.push(resource);
        }
    }
    resources
}

enum Projection<'a> {
    Manifest(&'a mut HelmManifestResourceSummary),
    Metadata(&'a mut HelmManifestResourceSummary),
    Text(&'a mut Option<String>),
    Discard,
}

impl<'de> DeserializeSeed<'de> for Projection<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Projection<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a YAML value")
    }

    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<(), A::Error> {
        // Keep the original mapping validation, including duplicate keys in discarded fields.
        // Only mapping keys and the projected identity strings need to be materialized.
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<serde_yaml::Value>()? {
            let field = match (&mut self, &key) {
                (Self::Manifest(resource), serde_yaml::Value::String(key)) => match key.as_str() {
                    "apiVersion" => Projection::Text(&mut resource.api_version),
                    "kind" => Projection::Text(&mut resource.kind),
                    "metadata" => Projection::Metadata(resource),
                    _ => Projection::Discard,
                },
                (Self::Metadata(resource), serde_yaml::Value::String(key)) => match key.as_str() {
                    "name" => Projection::Text(&mut resource.name),
                    "namespace" => Projection::Text(&mut resource.namespace),
                    _ => Projection::Discard,
                },
                _ => Projection::Discard,
            };
            if !keys.insert(key) {
                return Err(A::Error::custom("duplicate key in Helm manifest"));
            }
            map.next_value_seed(field)?;
        }
        Ok(())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq.next_element_seed(Self::Discard)?.is_some() {}
        Ok(())
    }

    fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<(), A::Error> {
        data.variant::<IgnoredAny>()?.1.newtype_variant_seed(self)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        self.deserialize(deserializer)
    }

    fn visit_str<E>(self, value: &str) -> Result<(), E> {
        if let Self::Text(field) = self
            && !value.is_empty()
        {
            *field = Some(value.to_owned());
        }
        Ok(())
    }

    fn visit_none<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<(), E> {
        Ok(())
    }

    fn visit_bool<E>(self, _: bool) -> Result<(), E> {
        Ok(())
    }

    fn visit_i64<E>(self, _: i64) -> Result<(), E> {
        Ok(())
    }

    fn visit_u64<E>(self, _: u64) -> Result<(), E> {
        Ok(())
    }

    fn visit_f64<E>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
}
