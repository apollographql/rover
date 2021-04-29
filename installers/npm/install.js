#!/usr/bin/env node

const { install } = require("./binary");
install();

// use setTimeout so the message prints after the install happens.zzs
setTimeout(() => {
  // these messages are duplicated in `src/command/install/mod.rs`
  // for the curl installer.
  console.log(
    "If you would like to disable Rover's anonymized usage collection, you can set APOLLO_TELEMETRY_DISABLED=1"
  );
  console.log(
    "You can check out our documentation at https://go.apollo.dev/r/docs."
  ),
    400;
});
