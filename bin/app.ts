#!/usr/bin/env node
import 'source-map-support/register';
import { App, Aspects } from 'aws-cdk-lib';
import { AwsSolutionsChecks } from 'cdk-nag';

import { CyberSageDataStack } from '../lib/cybersage-data-stack';
import { CyberSageControlStack } from '../lib/cybersage-control-stack';

const app = new App();

const env = {
  ...(process.env.CDK_DEFAULT_ACCOUNT === undefined
    ? {}
    : { account: process.env.CDK_DEFAULT_ACCOUNT }),
  ...(process.env.CDK_DEFAULT_REGION === undefined
    ? {}
    : { region: process.env.CDK_DEFAULT_REGION }),
};

const dataStack = new CyberSageDataStack(app, 'S-CyberSageDataStack', {
  env,
});

new CyberSageControlStack(app, 'S-CyberSageControlStack', {
  env,
  mainTable: dataStack.mainTable,
});

Aspects.of(app).add(new AwsSolutionsChecks({ verbose: true }));

app.synth();
