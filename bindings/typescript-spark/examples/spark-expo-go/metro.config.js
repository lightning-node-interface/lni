const path = require('node:path');
const { getDefaultConfig } = require('expo/metro-config');

const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, '../..');

const config = getDefaultConfig(projectRoot);

config.watchFolders = [workspaceRoot];
config.resolver.unstable_enableSymlinks = false;
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(workspaceRoot, 'node_modules'),
];
config.resolver.resolveRequest = (context, moduleName, platform) => {
  if (moduleName.startsWith('./dist/')) {
    const modulePath = moduleName.slice(2);
    const rootCandidate = path.resolve(workspaceRoot, modulePath);
    const rootCandidateJs = `${rootCandidate}.js`;

    if (require('node:fs').existsSync(rootCandidateJs)) {
      return {
        type: 'sourceFile',
        filePath: rootCandidateJs,
      };
    }

    if (require('node:fs').existsSync(rootCandidate)) {
      return {
        type: 'sourceFile',
        filePath: rootCandidate,
      };
    }
  }

  return context.resolveRequest(context, moduleName, platform);
};

module.exports = config;
