"use strict";

const axios = require("axios");
const cTable = require("console.table");
const libc = require("detect-libc");
const os = require("os");
const tar = require("tar");
const { configureProxy } = require("axios-proxy-builder");
const { existsSync, mkdirSync, rmSync } = require("fs");
const { join } = require("path");
const { spawnSync } = require("child_process");

const error = (msg) => {
  console.error(msg);
  process.exit(1);
};

const { version } = require("./package.json");
const fs = require("fs");
const name = `rover`;

const supportedPlatforms = [
  {
    TYPE: "Windows_NT",
    ARCHITECTURE: "x64",
    RUST_TARGET: "x86_64-pc-windows-msvc",
    BINARY_NAME: `${name}-${version}.exe`,
    RAW_NAME: `${name}.exe`
  },
  {
    TYPE: "Linux",
    ARCHITECTURE: "x64",
    RUST_TARGET: "x86_64-unknown-linux-gnu",
    BINARY_NAME: `${name}-${version}`,
    RAW_NAME: `${name}`
  },
  {
    TYPE: "Linux",
    ARCHITECTURE: "arm64",
    RUST_TARGET: "aarch64-unknown-linux-gnu",
    BINARY_NAME: `${name}-${version}`,
    RAW_NAME: `${name}`
  },
  {
    TYPE: "Darwin",
    ARCHITECTURE: "x64",
    RUST_TARGET: "x86_64-apple-darwin",
    BINARY_NAME: `${name}-${version}`,
    RAW_NAME: `${name}`
  },
  {
    TYPE: "Darwin",
    ARCHITECTURE: "arm64",
    RUST_TARGET: "aarch64-apple-darwin",
    BINARY_NAME: `${name}-${version}`,
    RAW_NAME: `${name}`
  },
];

const getPlatform = (type = os.type(), architecture = os.arch()) => {
  for (let supportedPlatform of supportedPlatforms) {
    if (
      type === supportedPlatform.TYPE &&
      architecture === supportedPlatform.ARCHITECTURE
    ) {
      if (supportedPlatform.TYPE === "Linux") {
        let musl_warning =
          "Downloading musl binary that does not include `rover supergraph compose`.";
        if (libc.isNonGlibcLinuxSync()) {
          console.warn(
            "This operating system does not support dynamic linking to glibc."
          );
          console.warn(musl_warning);
          supportedPlatform.RUST_TARGET = "x86_64-unknown-linux-musl";
        } else {
          let libc_version = libc.versionSync();
          let split_libc_version = libc_version.split(".");
          let libc_major_version = split_libc_version[0];
          let libc_minor_version = split_libc_version[1];
          let min_major_version = 2;
          let min_minor_version = 17;
          if (
            libc_major_version < min_major_version ||
            libc_minor_version < min_minor_version
          ) {
            console.warn(
              `This operating system needs glibc >= ${min_major_version}.${min_minor_version}, but only has ${libc_version} installed.`
            );
            console.warn(musl_warning);
            supportedPlatform.RUST_TARGET = "x86_64-unknown-linux-musl";
          }
        }
      }
      return supportedPlatform;
    }
  }

  error(
    `Platform with type "${type}" and architecture "${architecture}" is not supported by ${name}.\nYour system must be one of the following:\n\n${cTable.getTable(
      supportedPlatforms
    )}`
  );
};

/*! Copyright (c) 2019 Avery Harnish - MIT License */
class Binary {
  constructor(name, raw_name, url, installDirectory) {
    let errors = [];
    if (typeof url !== "string") {
      errors.push("url must be a string");
    } else {
      try {
        new URL(url);
      } catch (e) {
        errors.push(e);
      }
    }
    if (name && typeof name !== "string") {
      errors.push("name must be a string");
    }

    if (!name) {
      errors.push("You must specify the name of your binary");
    }
    if (errors.length > 0) {
      let errorMsg =
        "One or more of the parameters you passed to the Binary constructor are invalid:\n";
      errors.forEach(error => {
        errorMsg += error;
      });
      errorMsg +=
        '\n\nCorrect usage: new Binary("my-binary", "https://example.com/binary/download.tar.gz")';
      error(errorMsg);
    }
    this.url = url;
    this.name = name;
    this.raw_name = raw_name;
    this.installDirectory = installDirectory;

    if (!existsSync(this.installDirectory)) {
      mkdirSync(this.installDirectory, { recursive: true });
    }

    this.binaryPath = join(this.installDirectory, this.name);
  }

  exists() {
    return existsSync(this.binaryPath);
  }

  install(fetchOptions, suppressLogs = false) {
    if (this.exists()) {
      if (!suppressLogs) {
        console.error(
          `${this.name} is already installed, skipping installation.`
        );
      }
      return Promise.resolve();
    }

    if (existsSync(this.installDirectory)) {
      rmSync(this.installDirectory, { recursive: true });
    }

    mkdirSync(this.installDirectory, { recursive: true });

    if (!suppressLogs) {
      console.error(`Downloading release from ${this.url}`);
    }

    return axios({ ...fetchOptions, url: this.url, responseType: "stream" })
      .then(res => {
        return new Promise((resolve, reject) => {
          const sink = res.data.pipe(
            tar.x({ strip: 1, C: this.installDirectory })
          );
          sink.on("finish", () => resolve());
          sink.on("error", err => reject(err));
        });
      })
      .then(() => {
        fs.renameSync(join(this.installDirectory, this.raw_name), this.binaryPath);
        if (!suppressLogs) {
          console.error(`${this.name} has been installed!`);
        }
      })
      .catch(e => {
        error(`Error fetching release: ${e.message}`);
      });
  }

  run(fetchOptions) {
    const promise = !this.exists()
      ? this.install(fetchOptions, true)
      : Promise.resolve();

    promise
      .then(() => {
        const [, , ...args] = process.argv;

        const options = { cwd: process.cwd(), stdio: "inherit" };

        const result = spawnSync(this.binaryPath, args, options);

        if (result.error) {
          error(result.error);
        }

        process.exit(result.status);
      })
      .catch(e => {
        error(e.message);
        process.exit(1);
      });
  }
}

const getBinary = (overrideInstallDirectory, platform = getPlatform()) => {
  const download_host = process.env.npm_config_apollo_rover_download_host || process.env.APOLLO_ROVER_DOWNLOAD_HOST || 'https://rover.apollo.dev'
  // the url for this binary is constructed from values in `package.json`
  // https://rover.apollo.dev/tar/rover/x86_64-unknown-linux-gnu/v0.4.8
  const url = `${download_host}/tar/${name}/${platform.RUST_TARGET}/v${version}`;
  const { dirname } = require('path');
  const appDir = dirname(require.main.filename);
  let installDirectory = join(appDir, "binary");
  if (overrideInstallDirectory != null && overrideInstallDirectory !== "") {
    installDirectory = overrideInstallDirectory
  }
  let binary = new Binary(platform.BINARY_NAME, platform.RAW_NAME, url, installDirectory);

  // setting this allows us to extract supergraph plugins to the proper directory
  // the variable itself is read in Rust code
  process.env.APOLLO_NODE_MODULES_BIN_DIR = binary.installDirectory;
  return binary;
};

const install = (suppressLogs = false) => {
  const binary = getBinary();
  const proxy = configureProxy(binary.url);
  // these messages are duplicated in `src/command/install/mod.rs`
  // for the curl installer.
  if (!suppressLogs) {
    console.error(
      "If you would like to disable Rover's anonymized usage collection, you can set APOLLO_TELEMETRY_DISABLED=true"
    );
    console.error(
      "You can check out our documentation at https://go.apollo.dev/r/docs."
    );
  }

  return binary.install(proxy, suppressLogs);
};

const run = () => {
  const binary = getBinary();
  binary.run();
};

module.exports = {
  install,
  run,
  getBinary,
  getPlatform,
};
