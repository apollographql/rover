#!/usr/bin/env node

const { install } = require("./binary");
install();
// this is duplicated in `src/command/install/mod.rs`
// for the curl installer.

// use setTimeout so the message prints after the install happens.zzs
setTimeout(
  () =>
    console.log(
      "You can check out our documentation at https://go.apollo.dev/r/docs."
    ),
  400
);
