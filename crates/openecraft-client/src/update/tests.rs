use super::*;

fn v(text: &str) -> Version {
    Version::parse(text).unwrap()
}

#[test]
fn versions_are_parsed_from_tags() {
    assert_eq!(v("v0.2.1"), Version { major: 0, minor: 2, patch: 1, pre: String::new() });
    assert_eq!(v("1.0.0-rc.1+build.5").pre, "rc.1");
    assert_eq!(v(" 1.2.3 ").to_string(), "1.2.3");
    assert_eq!(v("v0.2.0-rc.1").to_string(), "0.2.0-rc.1");
    for bad in ["", "v1", "1.2", "1.2.3.4", "1.x.3", "latest"] {
        assert_eq!(Version::parse(bad), None, "{bad}");
    }
}

#[test]
fn versions_are_ordered_like_semver() {
    let order = ["0.1.0", "0.1.1", "0.2.0-alpha", "0.2.0-alpha.1", "0.2.0-alpha.beta", "0.2.0-beta.2", "0.2.0-beta.11", "0.2.0-rc.1", "0.2.0", "0.10.0", "1.0.0"];
    for pair in order.windows(2) {
        assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
    }
    assert_eq!(v("v1.0.0").cmp(&v("1.0.0+other")), Ordering::Equal);
}

#[test]
fn only_published_newer_releases_count() {
    let current = v("0.1.0");
    let release = |tag: &str| Release { tag_name: tag.into(), ..Default::default() };
    assert!(release("v0.1.1").is_newer_than(&current));
    assert!(!release("v0.1.0").is_newer_than(&current));
    assert!(!release("v0.0.9").is_newer_than(&current));
    assert!(!release("nightly").is_newer_than(&current));
    assert!(!Release { prerelease: true, ..release("v0.2.0-rc.1") }.is_newer_than(&current));
    assert!(!Release { draft: true, ..release("v0.2.0") }.is_newer_than(&current));
}

#[test]
fn the_release_answer_is_decoded() {
    let json = r#"{"tag_name":"v0.2.0","html_url":"https://github.com/nilres/OpenECraft/releases/tag/v0.2.0","draft":false,"prerelease":false,"body":"…",
        "assets":[{"name":"SHA256SUMS","browser_download_url":"https://example.org/SHA256SUMS","size":120,"content_type":"text/plain"}]}"#;
    let resp = ehttp::Response { url: String::new(), ok: true, status: 200, status_text: "OK".into(), headers: Default::default(), bytes: json.as_bytes().to_vec() };
    let release = latest(REPOSITORY).run(|_| Ok(resp.clone())).unwrap().unwrap();
    assert_eq!(release.version(), Some(v("0.2.0")));
    assert_eq!(release.asset(SUMS).map(|a| a.size), Some(120));
    assert!(release.asset("missing").is_none());
    // No release yet: nothing to update to.
    let none = ehttp::Response { url: String::new(), ok: false, status: 404, status_text: "Not Found".into(), headers: Default::default(), bytes: Vec::new() };
    assert_eq!(latest(REPOSITORY).run(|_| Ok(none.clone())).unwrap(), None);
    // Without a connection the check fails with a reason.
    assert!(latest(REPOSITORY).run(|_| Err("io: Connection refused".into())).is_err());
}

#[test]
fn the_check_asks_the_api_of_the_repository() {
    let call = latest("owner/repo");
    assert_eq!(call.request.url, "https://api.github.com/repos/owner/repo/releases/latest");
    assert_eq!(call.request.timeout, Some(CHECK_TIMEOUT));
    assert!(call.request.headers.get("User-Agent").is_some());
}

#[test]
fn file_names_follow_the_release_files() {
    let version = v("v0.2.0");
    assert_eq!(Package::WindowsSetup.file_name(&version, "x86_64"), "OpenECraft-0.2.0-setup.exe");
    assert_eq!(Package::AppImage.file_name(&version, "x86_64"), "OpenECraft-0.2.0-x86_64.AppImage");
    assert_eq!(Package::MacDiskImage.file_name(&version, "aarch64"), "OpenECraft-0.2.0-macos.dmg");
}

#[test]
fn downloads_are_checked_against_the_sums() {
    let data = b"abc";
    let hash = sha256(data);
    assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    let sums = format!("0000  other.zip\n{hash}  OpenECraft-0.2.0-setup.exe\n{hash} *binary.dmg\n");
    assert_eq!(checksum(&sums, "OpenECraft-0.2.0-setup.exe"), Some(hash.as_str()));
    assert_eq!(checksum(&sums, "binary.dmg"), Some(hash.as_str()));
    assert!(verify(data, &sums, "OpenECraft-0.2.0-setup.exe").is_ok());
    assert!(verify(&hash.to_uppercase().into_bytes(), &sums, "binary.dmg").is_err());
    assert!(verify(data, &sums, "other.zip").is_err());
    assert!(verify(data, &sums, "missing.exe").is_err());
}
