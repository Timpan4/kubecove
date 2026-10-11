use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinitionNames;
use kube::api::ObjectList;
use serde::{Deserialize, Deserializer};

use crate::models::DiscoveredResourceKind;

// Discovery needs no schemas, status or object metadata. Skipping them during
// decoding avoids allocating the large schema trees on every cache refresh.
#[derive(Clone, Default, Deserialize)]
pub(super) struct CatalogCrd {
    #[serde(default, rename = "apiVersion", deserialize_with = "crd_api_version")]
    _api_version: (),
    #[serde(default, rename = "kind", deserialize_with = "crd_kind")]
    _kind: (),
    #[serde(default, deserialize_with = "null_as_default")]
    pub spec: CatalogSpec,
}

fn crd_api_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    let value = String::deserialize(deserializer)?;
    if value == "apiextensions.k8s.io/v1" {
        Ok(())
    } else {
        Err(serde::de::Error::custom("invalid CRD apiVersion"))
    }
}

fn crd_kind<'de, D: Deserializer<'de>>(deserializer: D) -> Result<(), D::Error> {
    let value = String::deserialize(deserializer)?;
    if value == "CustomResourceDefinition" {
        Ok(())
    } else {
        Err(serde::de::Error::custom("invalid CRD kind"))
    }
}

#[derive(Clone, Default, Deserialize)]
pub(super) struct CatalogSpec {
    #[serde(default, deserialize_with = "null_as_default")]
    pub group: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub names: CustomResourceDefinitionNames,
    #[serde(default, deserialize_with = "null_as_default")]
    pub scope: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub versions: Vec<CatalogVersion>,
}

#[derive(Clone, Default, Deserialize)]
pub(super) struct CatalogVersion {
    #[serde(default, deserialize_with = "null_as_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_as_default")]
    pub served: bool,
    #[serde(default, deserialize_with = "null_as_default")]
    pub storage: bool,
}

fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(feature = "bench-support")]
impl From<k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition>
    for CatalogCrd
{
    fn from(
        crd: k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition,
    ) -> Self {
        Self {
            spec: CatalogSpec {
                group: crd.spec.group,
                names: crd.spec.names,
                scope: crd.spec.scope,
                versions: crd
                    .spec
                    .versions
                    .into_iter()
                    .map(|version| CatalogVersion {
                        name: version.name,
                        served: version.served,
                        storage: version.storage,
                    })
                    .collect(),
            },
            ..Self::default()
        }
    }
}

pub(crate) fn decode(text: &str) -> Result<Vec<DiscoveredResourceKind>, serde_json::Error> {
    let list: ObjectList<CatalogCrd> = serde_json::from_str(text)?;
    let mut kinds = list
        .items
        .iter()
        .filter_map(super::discovered_kind_from_crd)
        .collect();
    super::sort_and_dedup_kinds(&mut kinds);
    Ok(kinds)
}
