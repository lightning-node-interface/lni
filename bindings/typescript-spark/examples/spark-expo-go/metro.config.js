const path = require('node:path');
const { getDefaultConfig } = require('expo/metro-config');

const projectRoot = __dirname;
const sparkRoot = path.resolve(projectRoot, '../..');
const coreRoot = path.resolve(sparkRoot, '../typescript');
const config = getDefaultConfig(projectRoot);

// Both local file dependencies must be visible to Metro, including the core
// package outside the Spark directory. Modern Metro resolves their exports
// and symlinks without rewriting relative imports from other packages.
config.watchFolders = [sparkRoot, coreRoot];
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(sparkRoot, 'node_modules'),
  path.resolve(coreRoot, 'node_modules'),
];

module.exports = config;
