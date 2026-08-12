#!/usr/bin/env node
import 'source-map-support/register';
import { App } from 'aws-cdk-lib';

import { CyberSageDataStack } from '../lib/cybersage-data-stack';
import { CyberSageControlStack } from '../lib/cybersage-control-stack';

const app = new App();

const env = {
  account: process.env.CDK_DEFAULT_ACCOUNT,
  region: process.env.CDK_DEFAULT_REGION,
};

const dataStack = new CyberSageDataStack(app, 'S-CyberSageDataStack', {
  env,
});

new CyberSageControlStack(app, 'S-CyberSageControlStack', {
  env,
  mainTable: dataStack.mainTable,
});

app.synth();
