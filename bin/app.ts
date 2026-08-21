#!/usr/bin/env node
import 'source-map-support/register';
import { App, Aspects } from 'aws-cdk-lib';
import { AwsSolutionsChecks } from 'cdk-nag';
import { Code } from 'aws-cdk-lib/aws-lambda';
import { join } from 'node:path';

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
  lambdaCode: Code.fromAsset(join(__dirname, '../lambda/s-cybersage-rs/bootstrap.zip')),
});

Aspects.of(app).add(new AwsSolutionsChecks());

app.synth();
