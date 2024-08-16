const binary = require("../binary");
const os = require("os");
const path = require("path");
const pjson = require("../package.json");
const fs = require("node:fs");
const crypto = require("node:crypto");
const MockAdapter = require("axios-mock-adapter");
const axios = require("axios");
const {getPlatform} = require("../binary");

var mock = new MockAdapter(axios);
mock.onGet(new RegExp("https://rover\.apollo\.dev/tar/rover/x86_64-pc-windows-msvc/.*")).reply(function (_) {
  return [
    200,
    fs.createReadStream(
        path.join(__dirname, "fake_tarballs", "rover-fake-windows.tar.gz"),
    ),
  ];
});
mock.onGet(new RegExp("https://rover\.apollo\.dev/tar/rover/.*")).reply(function (_) {
  return [
    200,
    fs.createReadStream(
        path.join(__dirname, "fake_tarballs", "rover-fake.tar.gz"),
    ),
  ];
});


test("getBinary should be created with correct name and URL", () => {
  fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  const bin = binary.getBinary(os.tmpdir());
  expect(bin.name).toBe(`rover-${pjson.version}`);

  const platform = binary.getPlatform();
  expect(bin.url).toBe(
    `https://rover.apollo.dev/tar/rover/${platform.RUST_TARGET}/v${pjson.version}`,
  );
});

test("getBinary can override the installation directory", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  const bin = binary.getBinary(directory);
  expect(bin.installDirectory).toBe(directory);
});

test("getBinary creates the passed directory if it doesn't exist", () => {
  let old_tmp_directories = fs.readdirSync(os.tmpdir(), {
    withFileTypes: true,
  });
  let prefix = crypto.randomBytes(8).toString("hex");
  expect(
    old_tmp_directories.filter(
      (d) => d.isDirectory() && d.name.includes(prefix),
    ),
  ).toHaveLength(0);

  const directory = fs.mkdtempSync(path.join(os.tmpdir(), prefix));
  binary.getBinary(directory);

  let new_temp_directories = fs.readdirSync(os.tmpdir(), {
    withFileTypes: true,
  });
  expect(
    new_temp_directories.filter(
      (d) => d.isDirectory() && d.name.includes(prefix),
    ),
  ).toHaveLength(1);
});

test("getBinary creates a binary at the correct path", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  const bin = binary.getBinary(directory);
  expect(bin.binaryPath).toBe(path.join(directory, `rover-${pjson.version}`));
});

test("install doesn't do anything if a binary already exists", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  const bin = binary.getBinary(directory);

  const binary_name = `rover-${pjson.version}`;
  fs.writeFileSync(path.join(directory, binary_name), "foobarbash");
  bin.install({}, true);
  const file_contents = fs.readFileSync(path.join(directory, binary_name));
  expect(file_contents.toString()).toBe("foobarbash");
});

test("install recreates an existing directory if it exists", () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  const bin = binary.getBinary(directory);

  fs.writeFileSync(path.join(directory, "i-am-new-file.txt"), "foobarbash");
  fs.writeFileSync(
    path.join(directory, "i-am-a-different-new-file.txt"),
    "binboobaznar",
  );
  bin.install({}, true);

  const directory_entries = fs.readdirSync(directory, { withFileTypes: true });
  expect(
    directory_entries.filter((d) => d.isFile() && d.name.includes("i-am")),
  ).toHaveLength(0);
});

test("install downloads a binary if none exists", async () => {
  // Create temporary directory and binary
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  const bin = binary.getBinary(directory);
  //
  const directory_entries = await bin.install({}, true).then(async () => {
    return fs.readdirSync(directory, { withFileTypes: true });
  });
  const filtered_directory_entries = directory_entries.filter(
    (d) => d.isFile() && d.name === `rover-${pjson.version}`,
  );
  expect(filtered_directory_entries).toHaveLength(1);
  expect(
    fs.statSync(
      path.join(
        filtered_directory_entries[0].path,
        filtered_directory_entries[0].name,
      ),
    ).size,
  ).toBe(0);
});

test("install renames binary properly", async () => {
  // Establish temporary directory
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  // Create a Binary object
  const bin = binary.getBinary(directory);
  const directory_entries = await bin.install({}, true).then(async () => {
    return fs.readdirSync(directory, { withFileTypes: true });
  });
  expect(
    directory_entries.filter(
      (d) => d.isFile() && d.name === `rover-${pjson.version}`,
    ),
  ).toHaveLength(1);
});

test("install renames binary properly (Windows)", async () => {
  // Establish temporary directory
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  // Create a Binary object
  const bin = binary.getBinary(directory, getPlatform("Windows_NT", "x64"));
  const directory_entries = await bin.install({}, true).then(async () => {
    return fs.readdirSync(directory, { withFileTypes: true });
  });
  expect(
      directory_entries.filter(
          (d) => d.isFile() && d.name === `rover-${pjson.version}.exe`,
      ),
  ).toHaveLength(1);
});

test("install adds a new binary if another version exists", async () => {
  // Create the temporary directory
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), "rover-tests-"));
  // Put a fake binary into that directory, so it looks like one has been downloaded before
  const binary_name = `rover-0.22.0`;
  fs.writeFileSync(path.join(directory, binary_name), "foobarbash");
  // Check to ensure that we don't have a 0.23.0 version before we start
  const directory_entries = fs.readdirSync(directory, { withFileTypes: true });
  expect(
    directory_entries.filter(
      (d) => d.isFile() && d.name === `rover-${pjson.version}`,
    ),
  ).toHaveLength(0);

  // Create a Binary object
  const bin = binary.getBinary(directory);
  // Install the binary
  let new_directory_entries = await bin.install({}, true).then(() => {
    // Grab the directory entries again
    return fs.readdirSync(directory, { withFileTypes: true });
  });
  // Check that we now have a single file with the `rover` prefix
  expect(
    new_directory_entries.filter(
      (d) => d.isFile() && d.name.startsWith("rover-"),
    ),
  ).toHaveLength(1);
  // Check that the new version got downloaded
  expect(
    new_directory_entries.filter(
      (d) => d.isFile() && d.name === `rover-${pjson.version}`,
    ),
  ).toHaveLength(1);
});
