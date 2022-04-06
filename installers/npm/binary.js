const { Binary } = require("binary-install");
const os = require("os");
const cTable = require("console.table");
const libc = require("detect-libc");
const { join } = require("path");
const { spawnSync } = require("child_process");

const error = (msg) => {
  console.error(msg);
  process.exit(1);
};

const { version } = require("./package.json");
const name = "rover";

const supportedPlatforms = [
  {
    TYPE: "Windows_NT",
    ARCHITECTURE: "x64",
    RUST_TARGET: "x86_64-pc-windows-msvc",
    BINARY_NAME: `${name}.exe`,
  },
  {
    TYPE: "Linux",
    ARCHITECTURE: "x64",
    RUST_TARGET: "x86_64-unknown-linux-gnu",
    BINARY_NAME: name,
  },
  {
    TYPE: "Darwin",
    ARCHITECTURE: "x64",
    RUST_TARGET: "x86_64-apple-darwin",
    BINARY_NAME: name,
  },
  {
    TYPE: "Darwin",
    ARCHITECTURE: "arm64",
    RUST_TARGET: "x86_64-apple-darwin",
    BINARY_NAME: name,
  },
];

const getPlatform = () => {
  const type = os.type();
  const architecture = os.arch();

  for (supportedPlatform of supportedPlatforms) {
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

const getBinary = () => {
  const platform = getPlatform();
  // the url for this binary is constructed from values in `package.json`
  // https://rover.apollo.dev/tar/rover/x86_64-unknown-linux-gnu/v0.4.8
  const url = `https://rover.apollo.dev/tar/${name}/${platform.RUST_TARGET}/v${version}`;
  let binary = new Binary(platform.BINARY_NAME, url);

  // binary-install doesn't put the binary in the right place, so just patch it.
  binary.installDirectory = join(__dirname, "node_modules", ".bin");
  binary.binaryPath = join(binary.installDirectory, binary.name);
  // setting this allows us to extract supergraph plugins to the proper directory
  // the variable itself is read in Rust code
  process.env.APOLLO_NODE_MODULES_BIN_DIR = binary.installDirectory;
  return binary;
};

const run = () => {
  const binary = getBinary();
  binary.run();
};

const install = () => {
  const binary = getBinary();

  const proxy = configureProxy()

  binary.install(proxy);

  let pluginInstallCommand = `${binary.binaryPath} install --plugin`;
  let commands = [
    `${pluginInstallCommand} supergraph@latest-0`,
    `${pluginInstallCommand} supergraph@latest-2`,
  ];
  for (command of commands) {
    try {
      spawnSync(command, {
        stdio: "inherit",
        shell: true,
      });
    } catch (e) {
      console.error(
        `'${command.replace(
          binary.binaryPath,
          binary.name
        )}' failed with message '${
          e.message
        }'. 'rover supergraph compose' might not work properly on your machine.`
      );
    }
  }
};

const configureProxy = () => {
  // get proxy env
  const env = process.env.HTTP_PROXY || process.env.HTTPS_PROXY
  
   // short circuit if null 
  if (!env) return null
  
  // parse
  const {host, port, protocol, username, password} = new URL(env)

  // return proxy object for axios request
  return {
      proxy: {
        protocol,
        host,
        port,
        auth: {
          username,
          password
        }
      },
    };
}

module.exports = {
  install,
  run,
  getBinary,
};
