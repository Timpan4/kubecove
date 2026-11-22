use super::*;
use serde_json::json;

#[test]
fn formats_consolidated_asset_names_and_rejects_unknown_platforms() {
    let version = Version::new(0, 16, 0);
    for (platform, expected) in [
        ("linux-x86_64", Some("kubecove_0.16.0_x86_64.AppImage")),
        ("windows-x86_64", Some("kubecove_0.16.0_x64-setup.exe")),
        ("darwin-aarch64", Some("kubecove_0.16.0_aarch64.app.tar.gz")),
        ("darwin-x86_64", None),
    ] {
        assert_eq!(
            asset_name("kubecove", &version, platform).as_deref(),
            expected
        );
    }
}

#[test]
fn validates_product_identity_and_https_asset_urls() {
    let manifest = json!({
        "version": "0.16.0",
        "platforms": {
            "linux-x86_64": {
                "url": "https://example.test/kubecove_0.16.0_x86_64.AppImage",
                "signature": "signed"
            }
        }
    });
    validate_product(&manifest, "linux-x86_64", "KubeCove", "com.timpan.kubecove").unwrap();

    let mut wrong_id = manifest.clone();
    wrong_id["platforms"]["linux-x86_64"]["bundleId"] = "com.timpan.kubecove.gpui".into();
    assert!(
        validate_product(&wrong_id, "linux-x86_64", "KubeCove", "com.timpan.kubecove").is_err()
    );

    let mut wrong_name = manifest;
    wrong_name["productName"] = "KubeCove GPUI".into();
    assert!(
        validate_product(
            &wrong_name,
            "linux-x86_64",
            "KubeCove",
            "com.timpan.kubecove"
        )
        .is_err()
    );

    assert!(matches_asset(
        "https://example.test/kubecove_0.16.0_x86_64.AppImage",
        "kubecove_0.16.0_x86_64.AppImage",
    ));
    assert!(!matches_asset(
        "http://example.test/kubecove_0.16.0_x86_64.AppImage",
        "kubecove_0.16.0_x86_64.AppImage",
    ));
    assert!(!matches_asset(
        "https://example.test/kubecove-gpui_0.16.0_x86_64.AppImage",
        "kubecove_0.16.0_x86_64.AppImage",
    ));
}
