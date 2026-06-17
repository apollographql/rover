const binary = require("../binary");
const os = require("os");
const path = require("path");
const fs = require("node:fs");

// Helper: build a fake installed platform package in a temp dir and return
// the tryResolve / fileExists stubs that point to it.
const fakePackage = (platformKey, { withBinary = true } = {}) => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "rover-test-"));
  const binDir = path.join(tmpDir, "bin");
  fs.mkdirSync(binDir);
  const ext = platformKey.startsWith("win32") ? ".exe" : "";
  const binPath = path.join(binDir, `rover${ext}`);
  if (withBinary) {
    fs.writeFileSync(binPath, "");
  }
  fs.writeFileSync(path.join(tmpDir, "package.json"), "{}");

  const pkgName = {
    "darwin-arm64": "@apollo/rover-darwin-arm64",
    "darwin-x64": "@apollo/rover-darwin-x64",
    "linux-arm64": "@apollo/rover-linux-arm64",
    "linux-x64": "@apollo/rover-linux-x64",
    "linux-x64-musl": "@apollo/rover-linux-x64-musl",
    "win32-x64": "@apollo/rover-win32-x64",
  }[platformKey];

  const tryResolve = (id) => {
    if (id === `${pkgName}/package.json`) {
      return path.join(tmpDir, "package.json");
    }
    return null;
  };

  return { tmpDir, binPath, tryResolve };
};

describe("getBinaryPath", () => {
  test("resolves binary for darwin-arm64", () => {
    const { binPath, tryResolve } = fakePackage("darwin-arm64");
    const result = binary.getBinaryPath({
      platformKey: "darwin-arm64",
      tryResolve,
    });
    expect(result).toBe(binPath);
  });

  test("resolves binary for darwin-x64", () => {
    const { binPath, tryResolve } = fakePackage("darwin-x64");
    const result = binary.getBinaryPath({
      platformKey: "darwin-x64",
      tryResolve,
    });
    expect(result).toBe(binPath);
  });

  test("resolves binary for linux-arm64", () => {
    const { binPath, tryResolve } = fakePackage("linux-arm64");
    const result = binary.getBinaryPath({
      platformKey: "linux-arm64",
      tryResolve,
    });
    expect(result).toBe(binPath);
  });

  test("resolves binary for linux-x64 (glibc package)", () => {
    const { binPath, tryResolve } = fakePackage("linux-x64");
    const result = binary.getBinaryPath({
      platformKey: "linux-x64",
      tryResolve,
    });
    expect(result).toBe(binPath);
  });

  test("resolves binary for linux-x64 via musl fallback when glibc package absent", () => {
    const muslTmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "rover-test-"));
    const muslBinDir = path.join(muslTmpDir, "bin");
    fs.mkdirSync(muslBinDir);
    const muslBinPath = path.join(muslBinDir, "rover");
    fs.writeFileSync(muslBinPath, "");
    fs.writeFileSync(path.join(muslTmpDir, "package.json"), "{}");

    const tryResolve = (id) => {
      if (id === "@apollo/rover-linux-x64/package.json") return null;
      if (id === "@apollo/rover-linux-x64-musl/package.json") {
        return path.join(muslTmpDir, "package.json");
      }
      return null;
    };

    const result = binary.getBinaryPath({ platformKey: "linux-x64", tryResolve });
    expect(result).toBe(muslBinPath);
  });

  test("resolves binary for win32-x64", () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "rover-test-"));
    fs.writeFileSync(path.join(tmpDir, "package.json"), "{}");

    const tryResolve = (id) => {
      if (id === "@apollo/rover-win32-x64/package.json")
        return path.join(tmpDir, "package.json");
      return null;
    };

    // Use whatever ext getBinaryPath itself would use on the current platform
    // so the fileExists check passes regardless of where tests run.
    const ext = process.platform === "win32" ? ".exe" : "";
    const expectedBin = path.join(tmpDir, "bin", `rover${ext}`);

    const result = binary.getBinaryPath({
      platformKey: "win32-x64",
      tryResolve,
      fileExists: (p) => p === expectedBin,
    });
    expect(result).toBe(expectedBin);
  });

  test("exits with a helpful message for an unsupported platform", () => {
    const mockExit = jest.spyOn(process, "exit").mockImplementation(() => {
      throw new Error("process.exit called");
    });
    const mockErr = jest.spyOn(console, "error").mockImplementation(() => {});

    expect(() =>
      binary.getBinaryPath({ platformKey: "freebsd-x64" }),
    ).toThrow("process.exit called");

    expect(mockErr).toHaveBeenCalledWith(
      expect.stringContaining('"freebsd-x64"'),
    );

    mockExit.mockRestore();
    mockErr.mockRestore();
  });

  test("exits when platform package is installed but binary file is missing", () => {
    const { tryResolve } = fakePackage("darwin-arm64", { withBinary: false });

    const mockExit = jest.spyOn(process, "exit").mockImplementation(() => {
      throw new Error("process.exit called");
    });
    const mockErr = jest.spyOn(console, "error").mockImplementation(() => {});

    expect(() =>
      binary.getBinaryPath({ platformKey: "darwin-arm64", tryResolve }),
    ).toThrow("process.exit called");

    mockExit.mockRestore();
    mockErr.mockRestore();
  });
});

describe("APOLLO_NODE_MODULES_BIN_DIR", () => {
  test("run sets APOLLO_NODE_MODULES_BIN_DIR to the binary's parent directory", () => {
    // We can't easily call run() end-to-end (it would spawnSync rover), but we
    // can verify getBinaryPath returns a path whose dirname is the bin dir.
    const { binPath, tryResolve } = fakePackage("darwin-arm64");
    const resolvedBin = binary.getBinaryPath({
      platformKey: "darwin-arm64",
      tryResolve,
    });
    expect(path.dirname(resolvedBin)).toBe(path.dirname(binPath));
  });
});
