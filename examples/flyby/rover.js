require("dotenv").config()
const execSync = require('child_process').execSync;

if (!process.env.FLYBY_APOLLO_KEY) {
  throw "$FLYBY_APOLLO_KEY must be set to run this command"
}

process.env.APOLLO_KEY = process.env.FLYBY_APOLLO_KEY;


let command = "cargo rover";

const argv = process.argv;

const GRAPH_ID = "flyby-rover";

if (argv.length > 2) {
  const args = process.argv[2];
  command += ` ${args.replace("GRAPH_ID", GRAPH_ID)}`
}

console.error(`$ APOLLO_KEY=$FLYBY_APOLLO_KEY ${command}`)
try {
  execSync(command, { stdio: [0, 1, 2]} );
} catch {
  process.exit(1)
}