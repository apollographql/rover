#!/usr/bin/env node
'use strict'

// Injects APOLLO_NODE_MODULES_BIN_DIR into the cargo-npm generated shim so
// Rover's Rust code can locate supergraph plugin binaries at runtime.
const fs = require('fs')
const path = require('path')

const shimPath = path.join(__dirname, '../../installers/npm/@apollo/rover/bin/rover.js')
const content = fs.readFileSync(shimPath, 'utf8')

const patched = content.replace(
  'const bin = require.resolve(binPath)',
  "const bin = require.resolve(binPath)\nprocess.env.APOLLO_NODE_MODULES_BIN_DIR = require('path').dirname(bin)"
)

if (patched === content) {
  console.error('patch-npm-shim: marker not found — shim may have already been patched or changed format')
  process.exit(1)
}

fs.writeFileSync(shimPath, patched)
