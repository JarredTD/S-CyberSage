#!/usr/bin/env node
import "source-map-support/register";
import { App } from "aws-cdk-lib";
import { CyberSageCdkStack } from "../lib/cybersage-stack";
import { PaymentStack } from "../lib/payment-stack";

const app = new App();

const env = {
  account: process.env.CDK_DEFAULT_ACCOUNT,
  region: process.env.CDK_DEFAULT_REGION,
};

const paymentStack = new PaymentStack(app, "S-CyberSagePaymentStack", { env });

new CyberSageCdkStack(app, "S-CyberSageStack", {
  env,
  guildSubscriptionsTable: paymentStack.guildSubscriptionsTable,
});

app.synth();
