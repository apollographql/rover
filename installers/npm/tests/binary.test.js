const binary = require("../binary")
const os = require("os")
const path = require ("path")
const pjson = require('../package.json');
const fs = require("node:fs");
const fs_prom = require("node:fs/promises");
const crypto = require("node:crypto");
const {dirname} = require("path");
const MockAdapter = require("axios-mock-adapter");
const axios = require("axios");

test('getBinary should be created with correct name and URL', async () => {
    await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const bin = binary.getBinary(os.tmpdir());
    expect(bin.name).toBe(`rover-${pjson.version}`)

    const platform = binary.getPlatform();
    expect(bin.url).toBe(`https://rover.apollo.dev/tar/rover/${platform.RUST_TARGET}/v${pjson.version}`)
})

test('getBinary can override the installation directory', async () => {
    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const bin = binary.getBinary(directory);
    expect(bin.installDirectory).toBe(directory)
})

test('getBinary creates the passed directory if it doesn\'t exist', async () => {
    let old_tmp_directories = await fs_prom.readdir(os.tmpdir(), {withFileTypes: true});
    let prefix = crypto.randomBytes(8).toString('hex');
    expect(
        old_tmp_directories.filter(d => d.isDirectory() && d.name.includes(prefix))
    ).toHaveLength(0);

    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), prefix));
    const bin = binary.getBinary(directory);

    let new_temp_directories = await fs_prom.readdir(os.tmpdir(), {withFileTypes: true});
    expect(new_temp_directories.filter(d => d.isDirectory() && d.name.includes(prefix))).toHaveLength(1);
})

test('getBinary creates a binary at the correct path', async () => {
    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const bin = binary.getBinary(directory);
    expect(bin.binaryPath).toBe(path.join(directory, `rover-${pjson.version}`))
})

test('install doesn\'t do anything if a binary already exists', async () => {
    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const bin= binary.getBinary(directory);

    const binary_name = `rover-${pjson.version}`
    await fs_prom.writeFile(path.join(directory, binary_name), "foobarbash")
    bin.install({}, true);
    const file_contents = await fs_prom.readFile(path.join(directory, binary_name))
    expect(file_contents.toString()).toBe("foobarbash")
})

test('install recreates an existing directory if it exists', async () => {
    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const bin= binary.getBinary(directory);

    await fs_prom.writeFile(path.join(directory, "i-am-new-file.txt"), "foobarbash")
    await fs_prom.writeFile(path.join(directory, "i-am-a-different-new-file.txt"), "binboobaznar")
    bin.install({}, true);

    const directory_entries = await fs_prom.readdir(directory, {withFileTypes: true});
    expect(directory_entries.filter(d => d.isFile() && d.name.includes("i-am"))).toHaveLength(0);
})

test('install downloads a binary if none exists', async () => {
    var mock = new MockAdapter(axios);
    mock.onGet(new RegExp("https://rover\.apollo\.dev.*")).reply(function(config) {
        return [200, fs.createReadStream(path.join(__dirname, "fake_tarballs", "rover-fake.tar.gz"))];
    })

    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const bin= binary.getBinary(directory);
    bin.install({}, true).then(
        async () => {
            const directory_entries = await fs_prom.readdir(directory, {withFileTypes: true});
            expect(directory_entries.filter(d => d.isFile() && d.name === "rover-fake")).toHaveLength(1);
        }
    );
})

test('install adds a new binary if another version exists', async () => {
    var mock = new MockAdapter(axios);
    mock.onGet(new RegExp("https://rover\.apollo\.dev.*")).reply(function(config) {
        return [200, fs.createReadStream(path.join(__dirname, "fake_tarballs", "rover-fake.tar.gz"))];
    })
    const directory = await fs_prom.mkdtemp(path.join(os.tmpdir(), "rover-tests-"));
    const binary_name = `rover-0.22.0`
    await fs_prom.writeFile(path.join(directory, binary_name), "foobarbash")
    const directory_entries = await fs_prom.readdir(directory, {withFileTypes: true});
    expect(directory_entries.filter(d => d.isFile() && d.name === "rover-fake")).toHaveLength(0);

    const bin= binary.getBinary(directory);
    bin.install({}, true).then(
        async () => {
            const directory_entries = await fs_prom.readdir(directory, {withFileTypes: true});
            expect(directory_entries.filter(d => d.isFile() && d.name === "rover-fake")).toHaveLength(1);
        }
    );
})

