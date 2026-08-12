import { App } from 'aws-cdk-lib';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { Code } from 'aws-cdk-lib/aws-lambda';
import { describe, it } from 'vitest';

import { CyberSageControlStack } from '../lib/cybersage-control-stack';
import { CyberSageDataStack } from '../lib/cybersage-data-stack';

describe('CyberSageControlStack', () => {
  it('creates a constrained ARM Lambda behind a production HTTP API stage', () => {
    const app = new App();
    const dataStack = new CyberSageDataStack(app, 'S-CyberSageDataStack');
    const controlStack = new CyberSageControlStack(app, 'S-CyberSageControlStack', {
      mainTable: dataStack.mainTable,
      lambdaCode: Code.fromCfnParameters(),
    });
    const template = Template.fromStack(controlStack);

    template.hasResourceProperties('AWS::Lambda::Function', {
      Architectures: ['arm64'],
      MemorySize: 256,
      Timeout: 10,
      Runtime: 'provided.al2',
      Environment: {
        Variables: Match.objectLike({
          MAIN_TABLE_NAME: { 'Fn::ImportValue': Match.anyValue() },
          RUST_LOG: 'info',
        }),
      },
    });
    template.hasResourceProperties('AWS::ApiGatewayV2::Stage', {
      StageName: 'prod',
      AutoDeploy: true,
    });
    template.resourceCountIs('AWS::SecretsManager::Secret', 2);
  });
});
