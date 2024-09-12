require("dotenv").config();
const execSync = require("child_process").execSync;

if (!process.env.FLYBY_APOLLO_KEY) {
  throw "$FLYBY_APOLLO_KEY must be set to run this command";
}

process.env.APOLLO_KEY = process.env.FLYBY_APOLLO_KEY;
process.env.APOLLO_ELV2_LICENSE = "accept";

let command = process.env.ROVER_BINARY || "cargo rover";

const argv = process.argv;

const GRAPH_ID = "flyby-rover";

let should_fail = false;

if (argv.length > 2) {
  const args = argv[2];
  if (args.includes("SHOULD_FAIL")) {
    should_fail = true;
  }
  command += ` ${args
    .replace("GRAPH_ID", GRAPH_ID)
    .replace(" SHOULD_FAIL", "")}`;
}

console.error(`$ APOLLO_KEY=$FLYBY_APOLLO_KEY ${command}`);
try {
  execSync(command, { stdio: [0, 1, 2] });
} catch {
  if (should_fail) {
    console.error("command errored successfully");
    process.exit(0);
  } else {
    console.error(`${command} failed`);
    process.exit(1);
  }
}
