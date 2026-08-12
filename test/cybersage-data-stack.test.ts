import { App } from 'aws-cdk-lib';
import { Match, Template } from 'aws-cdk-lib/assertions';
import { describe, it } from 'vitest';

import { CyberSageDataStack } from '../lib/cybersage-data-stack';

describe('CyberSageDataStack', () => {
  it('creates an on-demand table with the required role indexes', () => {
    const app = new App();
    const stack = new CyberSageDataStack(app, 'Data');
    const template = Template.fromStack(stack);

    template.hasResourceProperties('AWS::DynamoDB::Table', {
      BillingMode: 'PAY_PER_REQUEST',
      KeySchema: [
        { AttributeName: 'PK', KeyType: 'HASH' },
        { AttributeName: 'SK', KeyType: 'RANGE' },
      ],
      GlobalSecondaryIndexes: Match.arrayWith([
        Match.objectLike({ IndexName: 'LookupByRoleName' }),
        Match.objectLike({ IndexName: 'LookupByRoleId' }),
        Match.objectLike({ IndexName: 'LookupByEntityType' }),
      ]),
    });
  });
});
