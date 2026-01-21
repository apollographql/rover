use std::{env, fs, io::Write, time::Duration};

use binstall::{download::FileDownloadService, Installer};
use camino::Utf8PathBuf;
use http::header::CONTENT_ENCODING;
use httpmock::prelude::*;
use reqwest::header::{ACCEPT, USER_AGENT};
use rover_http::ReqwestService;
use speculoos::prelude::*;

#[test]
pub fn test_install() {
    let mut executable_location =
        tempfile::NamedTempFile::new().expect("Unable to create temp file");
    executable_location
        .write_all("my plugin contents".as_bytes())
        .unwrap();
    executable_location.flush().unwrap();
    let executable_location_utf =
        Utf8PathBuf::from_path_buf(executable_location.path().to_path_buf())
            .expect("Unable to convert to Utf8PathBuf");
    let install_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let install_dir = Utf8PathBuf::from_path_buf(install_dir.path().to_path_buf())
        .expect("Unable to convert to Utf8PathBuf");
    let installer = Installer {
        binary_name: "test".to_string(),
        force_install: true, // necessary to bypass TTY prompt
        executable_location: executable_location_utf,
        override_install_path: Some(install_dir.clone()),
    };
    installer.install().unwrap();
    let expected_install_path = install_dir.join(".test").join("bin").join("test");
    let expected_install_path = if cfg!(windows) {
        expected_install_path.with_added_extension(env::consts::EXE_EXTENSION)
    } else {
        expected_install_path
    };
    let contents = fs::read_to_string(expected_install_path);
    assert_that!(contents)
        .is_ok()
        .is_equal_to("my plugin contents".to_string());
}

#[tokio::test]
pub async fn test_install_plugin() {
    let plugin_name = "my_plugin";
    let plugin_version = "v1.0.0";
    let plugin_contents = "my_plugin_contents";

    let override_path = tempfile::tempdir().expect("Unable to create temporary directory");
    let override_path = Utf8PathBuf::from_path_buf(override_path.path().to_path_buf())
        .expect("Unable to convert to Utf8PathBuf");

    let server = MockServer::start();
    let address = server.address();
    let _tarball_mock = server.mock(|when, then| {
        when.method(Method::GET)
            .path(format!("/{}", plugin_name))
            .header(USER_AGENT.as_str(), "rover-client")
            .header(ACCEPT.as_str(), "application/octet-stream");
        let gzipped_tar = gzipped_plugin_tarball(plugin_contents, plugin_name);
        then.status(200)
            .header(CONTENT_ENCODING.as_str(), "gzip")
            .body(&gzipped_tar[..]);
    });
    let _version_mock = server.mock(|when, then| {
        when.method(Method::HEAD).path(format!("/{}", plugin_name));
        then.status(200).header("x-version", "v1.0.0");
    });

    let tarball_url = format!("http://{}/{}", address, plugin_name);

    let install_subpath = format!(".{}", plugin_name);
    let bin_path = Utf8PathBuf::from(format!("{plugin_name}-{plugin_version}"));
    let bin_path = if cfg!(windows) {
        bin_path.with_added_extension(std::env::consts::EXE_EXTENSION)
    } else {
        bin_path
    };
    let expected_bin_path = override_path
        .join(install_subpath)
        .join("bin")
        .join(bin_path);

    let executable_location = tempfile::tempdir().unwrap();
    let executable_location = Utf8PathBuf::from_path_buf(executable_location.path().to_path_buf())
        .expect("Unable to convert to Utf8PathBuf");

    let installer = Installer {
        binary_name: plugin_name.to_string(),
        force_install: true,
        executable_location,
        override_install_path: Some(override_path),
    };

    let http_service = ReqwestService::builder()
        .client(reqwest::Client::new())
        .build()
        .unwrap();
    let service = FileDownloadService::builder()
        .http_service(http_service)
        .max_elapsed_duration(Duration::from_secs(5))
        .timeout_duration(Duration::from_secs(1))
        .build();

    let result = installer
        .install_plugin(plugin_name, &tarball_url, service, true)
        .await;

    assert_that!(result)
        .is_ok()
        .is_some()
        .is_equal_to(expected_bin_path);
}

fn gzipped_plugin_tarball(contents: &str, plugin_name: &str) -> Vec<u8> {
    let mut plugin_tempfile = tempfile::NamedTempFile::new().unwrap();
    plugin_tempfile.write_all(contents.as_bytes()).unwrap();
    plugin_tempfile.flush().unwrap();
    let plugin_subpath = if cfg!(windows) {
        format!("{}.{}", plugin_name, env::consts::EXE_EXTENSION)
    } else {
        plugin_name.to_string()
    };
    let mut builder = tar::Builder::new(Vec::new());
    builder
        .append_path_with_name(plugin_tempfile.path(), format!("dist/{}", plugin_subpath))
        .unwrap();
    let tar_bytes = builder.into_inner().unwrap();
    let mut gzip_encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip_encoder.write_all(&tar_bytes).unwrap();
    gzip_encoder.finish().unwrap()
}
